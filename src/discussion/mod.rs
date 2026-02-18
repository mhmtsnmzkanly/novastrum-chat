use std::cmp::Reverse;

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    AppState, auth,
    events::EventType,
    logging,
    models::{
        ApiEnvelope, CursorPage, DiscussionEditRequest, DiscussionEditStatus, DiscussionPost,
    },
    websocket::{WsAdminEvent, WsDiscussionPost, WsDiscussionVote, WsServerEvent},
};

/// Payload for creating a discussion thread/reply.
#[derive(Debug, Deserialize)]
pub struct CreatePostPayload {
    /// Author user id.
    pub user_id: Uuid,
    /// Optional thread id (auto-filled from parent for replies).
    pub thread_id: Option<Uuid>,
    /// Optional parent post id.
    pub parent_id: Option<Uuid>,
    /// Title used primarily for top-level threads.
    pub title: String,
    /// Post body.
    pub body: String,
}

/// Payload for cast/undo vote operation.
#[derive(Debug, Deserialize)]
pub struct VotePayload {
    /// Voter user id.
    pub user_id: Uuid,
    /// Target post id.
    pub post_id: Uuid,
    /// Vote value (`-1` or `1`).
    pub value: i8,
    /// Undo flag to remove existing vote.
    pub undo: Option<bool>,
}

/// Query params for discussion listing.
#[derive(Debug, Deserialize)]
pub struct ListPostsQuery {
    /// Optional thread filter.
    pub thread_id: Option<Uuid>,
    /// Sort mode: `hot`, `top`, `new`.
    pub sort: Option<String>,
    /// Optional pagination cursor (ISO timestamp).
    pub cursor: Option<String>,
    /// Optional page size limit.
    pub limit: Option<usize>,
    /// Optional parent filter for detail view (if provided, include its direct children).
    pub parent_id: Option<Uuid>,
}

/// Payload for requesting a post edit.
#[derive(Debug, Deserialize)]
pub struct RequestEditPayload {
    /// Author user id.
    pub user_id: Uuid,
    /// Post to edit.
    pub post_id: Uuid,
    /// New title (optional).
    pub new_title: Option<String>,
    /// New body content (required).
    pub new_body: String,
}

/// Creates discussion post while enforcing `max depth = 3`.
pub async fn create_post_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreatePostPayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }
    if payload.body.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "post body cannot be empty",
            )),
        );
    }

    // Validate author exists and can write.
    let author = state
        .db
        .read(|db| db.users.get(&payload.user_id).cloned())
        .await;
    let Some(author) = author else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("author not found")),
        );
    };
    if !author.status.can_write() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "author cannot write discussion content",
            )),
        );
    }

    let now = Utc::now();

    // Resolve parent/depth/thread relationship.
    let (thread_id, depth) = if let Some(parent_id) = payload.parent_id {
        let parent = state
            .db
            .read(|db| db.discussion_posts.get(&parent_id).cloned())
            .await;

        let Some(parent) = parent else {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "parent post not found",
                )),
            );
        };

        let depth = parent.depth.saturating_add(1);
        if depth > 3 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "max discussion depth is 3",
                )),
            );
        }

        (parent.thread_id, depth)
    } else {
        (payload.thread_id.unwrap_or_else(Uuid::new_v4), 0)
    };

    // Build post row.
    let post = DiscussionPost {
        id: Uuid::new_v4(),
        thread_id,
        parent_id: payload.parent_id,
        depth,
        author_id: payload.user_id,
        title: payload.title.trim().to_string(),
        body: payload.body.trim().to_string(),
        created_at: now,
        edited_at: None,
        score: 0,
    };

    // Insert post row.
    state
        .db
        .write(|db| {
            db.discussion_posts.insert(post.id, post.clone());
        })
        .await;

    // Emit event per post depth.
    let event_type = if depth == 0 {
        EventType::DiscussionCreated
    } else {
        EventType::DiscussionReplied
    };
    let event = state
        .events
        .insert_event_row(
            &state.db,
            event_type,
            json!({
                "post_id": post.id,
                "thread_id": post.thread_id,
                "parent_id": post.parent_id,
                "depth": post.depth,
            }),
        )
        .await;

    // Record activity log.
    logging::log_activity(
        &state.db,
        "discussion_post_created",
        Some(payload.user_id),
        json!({"post_id": post.id, "depth": post.depth}),
    )
    .await;

    state.events.dispatch_after_commit(event);
    state
        .websocket_broadcaster
        .publish(WsServerEvent::DiscussionPost(WsDiscussionPost::from(&post)));

    (
        StatusCode::OK,
        Json(ApiEnvelope::success("discussion post created", json!(post))),
    )
}

/// Requests a moderation review for editing an existing discussion post.
pub async fn request_edit_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RequestEditPayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }

    if payload.new_body.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "new body cannot be empty",
            )),
        );
    }

    let post = state
        .db
        .read(|db| db.discussion_posts.get(&payload.post_id).cloned())
        .await;
    let Some(post) = post else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("post not found")),
        );
    };

    if post.author_id != payload.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "only the author can request edits",
            )),
        );
    }

    let request = DiscussionEditRequest {
        id: Uuid::new_v4(),
        post_id: payload.post_id,
        author_id: payload.user_id,
        new_title: payload.new_title.map(|value| value.trim().to_string()),
        new_body: payload.new_body.trim().to_string(),
        status: DiscussionEditStatus::Pending,
        requested_at: Utc::now(),
        resolved_at: None,
        resolved_by: None,
    };

    let inserted = state
        .db
        .write(|db| {
            let already_pending = db.discussion_edit_requests.iter().any(|existing| {
                existing.post_id == request.post_id
                    && existing.status == DiscussionEditStatus::Pending
            });
            if already_pending {
                return false;
            }

            db.discussion_edit_requests.push(request.clone());
            true
        })
        .await;

    if !inserted {
        return (
            StatusCode::CONFLICT,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "pending edit request already exists",
            )),
        );
    }

    logging::log_activity(
        &state.db,
        "discussion_edit_requested",
        Some(payload.user_id),
        json!({"post_id": payload.post_id, "request_id": request.id}),
    )
    .await;

    let event = state
        .events
        .insert_event_row(
            &state.db,
            EventType::DiscussionEditRequested,
            json!({
                "request_id": request.id,
                "post_id": request.post_id,
                "author_id": request.author_id,
            }),
        )
        .await;
    state.events.dispatch_after_commit(event);

    state
        .websocket_broadcaster
        .publish(WsServerEvent::AdminEvent(WsAdminEvent {
            action: "discussion.edit.requested".to_string(),
            details: json!({
                "request_id": request.id,
                "post_id": request.post_id,
                "author_id": request.author_id,
            }),
        }));

    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "edit request submitted",
            json!(request),
        )),
    )
}

/// Casts or undoes vote while enforcing per-user vote rate.
pub async fn vote_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<VotePayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }
    // Validate vote target exists.
    let post_exists = state
        .db
        .read(|db| db.discussion_posts.contains_key(&payload.post_id))
        .await;
    if !post_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("post not found")),
        );
    }

    // Undo branch removes previous vote without requiring value checks.
    if payload.undo.unwrap_or(false) {
        state
            .db
            .write(|db| {
                db.discussion_votes
                    .remove(&(payload.post_id, payload.user_id));
            })
            .await;

        logging::log_activity(
            &state.db,
            "vote_undone",
            Some(payload.user_id),
            json!({"post_id": payload.post_id}),
        )
        .await;

        return (
            StatusCode::OK,
            Json(ApiEnvelope::success_no_data("vote undone")),
        );
    }

    // Validate allowed vote values.
    if payload.value != -1 && payload.value != 1 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "vote value must be -1 or 1",
            )),
        );
    }

    // Enforce user-based vote rate limit (`1 per minute`).
    if !state
        .rate_limit
        .allow_vote(payload.user_id, Utc::now())
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "vote rate limit exceeded",
            )),
        );
    }

    // Upsert vote row (1 vote per user per post).
    state
        .db
        .write(|db| {
            db.discussion_votes
                .insert((payload.post_id, payload.user_id), payload.value);
        })
        .await;

    // Emit vote event.
    let event = state
        .events
        .insert_event_row(
            &state.db,
            EventType::VoteCast,
            json!({
                "post_id": payload.post_id,
                "user_id": payload.user_id,
                "value": payload.value,
            }),
        )
        .await;
    state.events.dispatch_after_commit(event);

    logging::log_activity(
        &state.db,
        "vote_cast",
        Some(payload.user_id),
        json!({"post_id": payload.post_id, "value": payload.value}),
    )
    .await;

    let score = state
        .db
        .read(|db| {
            db.discussion_votes
                .iter()
                .filter(|((post_id, _), _)| *post_id == payload.post_id)
                .map(|(_, value)| i64::from(*value))
                .sum()
        })
        .await;

    state
        .websocket_broadcaster
        .publish(WsServerEvent::DiscussionVote(WsDiscussionVote {
            post_id: payload.post_id,
            score,
        }));

    (
        StatusCode::OK,
        Json(ApiEnvelope::success_no_data("vote saved")),
    )
}

/// Lists discussion posts sorted by `hot`, `top`, or `new`.
pub async fn list_posts_handler(
    State(state): State<AppState>,
    Query(query): Query<ListPostsQuery>,
) -> Json<ApiEnvelope<CursorPage<DiscussionPost>>> {
    let sort_mode = query
        .sort
        .as_deref()
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "hot".to_string());

    let cursor_threshold = query
        .cursor
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let limit = query.limit.unwrap_or(50).clamp(10, 200);

    let (mut posts, votes) = state
        .db
        .read(|db| {
            let mut posts = db
                .discussion_posts
                .values()
                .filter(|post| {
                    query
                        .thread_id
                        .is_none_or(|thread_id| post.thread_id == thread_id)
                        && query.parent_id.is_none_or(|pid| post.parent_id == Some(pid))
                })
                .cloned()
                .collect::<Vec<_>>();

            if let Some(cursor) = cursor_threshold {
                posts.retain(|post| post.created_at < cursor);
            }

            let votes = db.discussion_votes.clone();
            (posts, votes)
        })
        .await;

    match sort_mode.as_str() {
        "new" => {
            posts.sort_by_key(|post| Reverse(post.created_at));
        }
        "top" => {
            posts.sort_by_key(|post| {
                Reverse(
                    votes
                        .iter()
                        .filter(|((post_id, _), _)| *post_id == post.id)
                        .map(|(_, value)| i64::from(*value))
                        .sum::<i64>(),
                )
            });
        }
        _ => {
            posts.sort_by_key(|post| {
                let score = votes
                    .iter()
                    .filter(|((post_id, _), _)| *post_id == post.id)
                    .map(|(_, value)| i64::from(*value))
                    .sum::<i64>();
                let age_minutes = (Utc::now() - post.created_at).num_minutes().max(1);
                Reverse(score * 10 - age_minutes)
            });
        }
    }

    let limited = posts.into_iter().take(limit).collect::<Vec<_>>();
    let next_cursor = limited.last().map(|post| post.created_at.to_rfc3339());

    Json(ApiEnvelope::success(
        "discussion posts listed",
        CursorPage {
            items: limited,
            next_cursor,
        },
    ))
}
