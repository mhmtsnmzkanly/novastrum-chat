use std::collections::HashSet;

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    AppState, auth,
    db::{ActivityLogRow, DbEventRow},
    events::EventType,
    logging,
    models::{ApiEnvelope, DiscussionEditStatus, UserStatus},
    permissions,
    rate_limit::RateConfig,
    websocket::{WsAdminEvent, WsServerEvent},
};

/// Admin view row used by user management table.
#[derive(Debug, Clone, Serialize)]
pub struct AdminUserView {
    /// User id.
    pub id: Uuid,
    /// Login username.
    pub login_username: String,
    /// Public username.
    pub public_username: String,
    /// Numeric status value.
    pub status: u8,
    /// Human-readable status label.
    pub status_label: String,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: chrono::DateTime<Utc>,
    /// Assigned permissions.
    pub permissions: Vec<String>,
    /// Active device/session count.
    pub active_sessions: usize,
}

/// Overview metrics shown in admin dashboard cards.
#[derive(Debug, Clone, Serialize)]
pub struct AdminMetrics {
    /// Total users in system.
    pub total_users: usize,
    /// Pending users waiting approval.
    pub pending_users: usize,
    /// Active users.
    pub active_users: usize,
    /// Suspended users.
    pub suspended_users: usize,
    /// Banned users.
    pub banned_users: usize,
    /// Total DB-backed sessions.
    pub total_sessions: usize,
    /// Total chat rooms.
    pub total_chats: usize,
    /// Total message rows across all chat tables.
    pub total_messages: usize,
    /// Total event rows.
    pub total_events: usize,
    /// Total activity logs.
    pub total_logs: usize,
    /// Total pending DM requests.
    pub pending_dm_requests: usize,
}

/// Response payload for admin overview endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct AdminOverview {
    /// Authenticated admin user id.
    pub admin_user_id: Uuid,
    /// Authenticated admin login.
    pub admin_login: String,
    /// Aggregated system metrics.
    pub metrics: AdminMetrics,
    /// Pending approval user rows.
    pub pending_approvals: Vec<AdminUserView>,
    /// Recent events for monitoring.
    pub recent_events: Vec<DbEventRow>,
    /// Recent logs for monitoring.
    pub recent_logs: Vec<ActivityLogRow>,
    /// Runtime rate-limit configuration.
    pub rate_config: RateConfig,
}

/// Query params for admin user listing.
#[derive(Debug, Deserialize)]
pub struct AdminListUsersQuery {
    /// Optional status filter.
    pub status: Option<u8>,
    /// Optional search filter against login/public usernames.
    pub search: Option<String>,
}

/// Payload for approval action.
#[derive(Debug, Deserialize)]
pub struct AdminApprovePayload {
    /// Target user id to approve.
    pub user_id: Uuid,
}

/// Payload for hard reject action.
#[derive(Debug, Deserialize)]
pub struct AdminRejectPayload {
    /// Target user id to hard delete.
    pub user_id: Uuid,
}

/// Payload for status update action.
#[derive(Debug, Deserialize)]
pub struct AdminSetStatusPayload {
    /// Target user id.
    pub user_id: Uuid,
    /// New status value (`1..=4`).
    pub status: u8,
}

/// Payload for permission toggle action.
#[derive(Debug, Deserialize)]
pub struct AdminPermissionTogglePayload {
    /// Target user id.
    pub user_id: Uuid,
    /// Permission key.
    pub permission: String,
    /// Whether permission should be enabled.
    pub enabled: bool,
}

/// Payload for updating runtime rate limits.
#[derive(Debug, Deserialize)]
pub struct AdminUpdateRatePayload {
    /// Optional replacement for message interval seconds.
    pub message_every_seconds: Option<i64>,
    /// Optional replacement for vote interval seconds.
    pub upvote_every_seconds: Option<i64>,
    /// Optional replacement for DM per minute limit.
    pub dm_per_minute: Option<usize>,
    /// Optional replacement for DM per hour limit.
    pub dm_per_hour: Option<usize>,
    /// Optional replacement for DM per day limit.
    pub dm_per_day: Option<usize>,
}

/// Query params for audit endpoint.
#[derive(Debug, Deserialize)]
pub struct AdminAuditQuery {
    /// Max row count for events/logs list.
    pub limit: Option<usize>,
}

/// Audit payload used by admin panel.
#[derive(Debug, Clone, Serialize)]
pub struct AdminAuditPayload {
    /// Recent event rows.
    pub events: Vec<DbEventRow>,
    /// Recent activity log rows.
    pub logs: Vec<ActivityLogRow>,
}

/// Query params for discussion edit moderation list.
#[derive(Debug, Deserialize)]
pub struct AdminDiscussionEditQuery {
    /// Optional filter by request status.
    pub status: Option<u8>,
}

/// Payload for approving or rejecting an edit request.
#[derive(Debug, Deserialize)]
pub struct AdminDiscussionEditDecisionPayload {
    /// Target request id.
    pub request_id: Uuid,
    /// Whether to approve (true) or reject (false).
    pub approve: bool,
}

/// View model for edit requests shown on admin panel.
#[derive(Debug, Clone, Serialize)]
pub struct AdminDiscussionEditView {
    pub request_id: Uuid,
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub author_login: Option<String>,
    pub current_status: u8,
    pub status_label: String,
    pub new_title: Option<String>,
    pub new_body: String,
    pub requested_at: DateTime<Utc>,
}

/// Returns dashboard-level metrics and pending approval list.
pub async fn overview_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let admin_user = match require_admin_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    // Build overview data from a single state snapshot read.
    let (metrics, pending_approvals, recent_events, recent_logs) = state
        .db
        .read(|db| {
            let total_users = db.users.len();
            let pending_users = db
                .users
                .values()
                .filter(|user| user.status == UserStatus::Pending)
                .count();
            let active_users = db
                .users
                .values()
                .filter(|user| user.status == UserStatus::Active)
                .count();
            let suspended_users = db
                .users
                .values()
                .filter(|user| user.status == UserStatus::Suspended)
                .count();
            let banned_users = db
                .users
                .values()
                .filter(|user| user.status == UserStatus::Banned)
                .count();

            let metrics = AdminMetrics {
                total_users,
                pending_users,
                active_users,
                suspended_users,
                banned_users,
                total_sessions: db.sessions.len(),
                total_chats: db.chats.len(),
                total_messages: db.dm_messages.len()
                    + db.group_messages.len()
                    + db.community_messages.len(),
                total_events: db.events.len(),
                total_logs: db.activity_logs.len(),
                pending_dm_requests: db.pending_dm_requests.len(),
            };

            let pending_approvals = build_user_views(db)
                .into_iter()
                .filter(|user| user.status == UserStatus::Pending as u8)
                .take(25)
                .collect::<Vec<_>>();

            let recent_events = db.events.iter().rev().take(20).cloned().collect::<Vec<_>>();
            let recent_logs = db
                .activity_logs
                .iter()
                .rev()
                .take(20)
                .cloned()
                .collect::<Vec<_>>();

            (metrics, pending_approvals, recent_events, recent_logs)
        })
        .await;

    let rate_config = state.rate_limit.get_config().await;

    Json(ApiEnvelope::success(
        "admin overview fetched",
        AdminOverview {
            admin_user_id: admin_user.id,
            admin_login: admin_user.login_username,
            metrics,
            pending_approvals,
            recent_events,
            recent_logs,
            rate_config,
        },
    ))
    .into_response()
}

/// Returns user list with optional status/search filters.
pub async fn list_users_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AdminListUsersQuery>,
) -> impl IntoResponse {
    if let Err(response) = require_admin_user(&state, &headers).await {
        return response;
    }

    let search = query.search.as_deref().map(str::to_ascii_lowercase);

    let users = state
        .db
        .read(|db| {
            build_user_views(db)
                .into_iter()
                .filter(|user| query.status.is_none_or(|status| user.status == status))
                .filter(|user| {
                    search.as_ref().is_none_or(|search| {
                        user.login_username.contains(search)
                            || user.public_username.contains(search)
                    })
                })
                .collect::<Vec<_>>()
        })
        .await;

    Json(ApiEnvelope::success("admin users fetched", users)).into_response()
}

/// Approves a pending user by setting status to `Active`.
pub async fn approve_user_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AdminApprovePayload>,
) -> impl IntoResponse {
    let admin_user = match require_admin_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let updated = state
        .db
        .write(|db| {
            let Some(target) = db.users.get_mut(&payload.user_id) else {
                return None;
            };

            target.status = UserStatus::Active;
            target.updated_at = Utc::now();

            Some(target.login_username.clone())
        })
        .await;

    let Some(login_username) = updated else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "target user not found",
            )),
        )
            .into_response();
    };

    logging::log_activity(
        &state.db,
        "admin_user_approved",
        Some(admin_user.id),
        json!({"target_user_id": payload.user_id, "login_username": login_username}),
    )
    .await;

    Json(ApiEnvelope::<serde_json::Value>::success_no_data(
        "user approved and activated",
    ))
    .into_response()
}

/// Applies one of the allowed status values (`Pending|Active|Suspended|Banned`).
pub async fn set_user_status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AdminSetStatusPayload>,
) -> impl IntoResponse {
    let admin_user = match require_admin_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let new_status = match UserStatus::try_from(payload.status) {
        Ok(status) => status,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "status must be one of 1,2,3,4",
                )),
            )
                .into_response();
        }
    };

    // Prevent admin from self-locking by accident.
    if payload.user_id == admin_user.id && new_status != UserStatus::Active {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "admin cannot set own status to non-active",
            )),
        )
            .into_response();
    }

    let updated = state
        .db
        .write(|db| {
            let Some(target) = db.users.get_mut(&payload.user_id) else {
                return None;
            };

            target.status = new_status;
            target.updated_at = Utc::now();

            Some(target.login_username.clone())
        })
        .await;

    let Some(login_username) = updated else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "target user not found",
            )),
        )
            .into_response();
    };

    logging::log_activity(
        &state.db,
        "admin_user_status_updated",
        Some(admin_user.id),
        json!({
            "target_user_id": payload.user_id,
            "login_username": login_username,
            "new_status": payload.status
        }),
    )
    .await;

    Json(ApiEnvelope::<serde_json::Value>::success_no_data(
        "user status updated",
    ))
    .into_response()
}

/// Hard-deletes a user account and associated rows (reject flow from specification).
pub async fn reject_user_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AdminRejectPayload>,
) -> impl IntoResponse {
    let admin_user = match require_admin_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    // Prevent deleting currently authenticated admin account.
    if payload.user_id == admin_user.id {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "admin cannot reject own account",
            )),
        )
            .into_response();
    }

    let deleted_login = state
        .db
        .write(|db| {
            let target = db.users.remove(&payload.user_id)?;

            // Remove username indexes.
            db.users_by_login.remove(&target.login_username);
            db.users_by_public.remove(&target.public_username);

            // Remove all sessions for rejected account.
            db.sessions
                .retain(|_, session| session.user_id != payload.user_id);

            // Remove permission grants and friendship edges.
            db.user_permissions.remove(&payload.user_id);
            db.friendships
                .retain(|(a, b)| *a != payload.user_id && *b != payload.user_id);

            // Remove pending DM requests involving this user.
            db.pending_dm_requests.retain(|row| {
                row.from_user_id != payload.user_id && row.to_user_id != payload.user_id
            });

            // Remove chats created by this user and capture removed ids.
            let removed_chat_ids = db
                .chats
                .iter()
                .filter(|(_, chat)| chat.created_by == payload.user_id)
                .map(|(chat_id, _)| *chat_id)
                .collect::<HashSet<_>>();
            db.chats
                .retain(|chat_id, _| !removed_chat_ids.contains(chat_id));

            // Remove memberships and chat messages linked to removed user/chats.
            db.chat_members.retain(|row| {
                row.user_id != payload.user_id && !removed_chat_ids.contains(&row.chat_id)
            });
            db.dm_messages.retain(|row| {
                row.sender_id != payload.user_id && !removed_chat_ids.contains(&row.chat_id)
            });
            db.group_messages.retain(|row| {
                row.sender_id != payload.user_id && !removed_chat_ids.contains(&row.chat_id)
            });
            db.community_messages.retain(|row| {
                row.sender_id != payload.user_id && !removed_chat_ids.contains(&row.chat_id)
            });

            // Remove discussion posts and votes linked to rejected user.
            let removed_post_ids = db
                .discussion_posts
                .iter()
                .filter(|(_, post)| post.author_id == payload.user_id)
                .map(|(post_id, _)| *post_id)
                .collect::<HashSet<_>>();
            db.discussion_posts
                .retain(|post_id, _| !removed_post_ids.contains(post_id));
            db.discussion_votes.retain(|(post_id, voter_id), _| {
                *voter_id != payload.user_id && !removed_post_ids.contains(post_id)
            });

            // Remove notifications/files for this user or removed chats.
            db.notifications.retain(|row| {
                row.user_id != payload.user_id
                    && row
                        .chat_id
                        .is_none_or(|chat_id| !removed_chat_ids.contains(&chat_id))
            });
            db.files.retain(|_, file| file.owner_id != payload.user_id);

            Some(target.login_username)
        })
        .await;

    let Some(login_username) = deleted_login else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "target user not found",
            )),
        )
            .into_response();
    };

    logging::log_activity(
        &state.db,
        "admin_user_rejected_hard_deleted",
        Some(admin_user.id),
        json!({"target_user_id": payload.user_id, "login_username": login_username}),
    )
    .await;

    Json(ApiEnvelope::<serde_json::Value>::success_no_data(
        "user hard-deleted",
    ))
    .into_response()
}

/// Toggles a permission key for a user.
pub async fn toggle_permission_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AdminPermissionTogglePayload>,
) -> impl IntoResponse {
    let admin_user = match require_admin_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    if !allowed_permission_keys()
        .iter()
        .any(|permission| *permission == payload.permission)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "unsupported permission key",
            )),
        )
            .into_response();
    }

    let updated_permissions = state
        .db
        .write(|db| {
            if !db.users.contains_key(&payload.user_id) {
                return None;
            }

            let permissions = db.user_permissions.entry(payload.user_id).or_default();
            if payload.enabled {
                permissions.insert(payload.permission.clone());
            } else {
                permissions.remove(payload.permission.as_str());
            }

            Some(permissions.iter().cloned().collect::<Vec<_>>())
        })
        .await;

    let Some(updated_permissions) = updated_permissions else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "target user not found",
            )),
        )
            .into_response();
    };

    logging::log_activity(
        &state.db,
        "admin_permission_toggled",
        Some(admin_user.id),
        json!({
            "target_user_id": payload.user_id,
            "permission": payload.permission,
            "enabled": payload.enabled,
        }),
    )
    .await;

    state
        .websocket_broadcaster
        .publish(WsServerEvent::AdminEvent(WsAdminEvent {
            action: "admin.permission.changed".to_string(),
            details: json!({
                "target_user_id": payload.user_id,
                "permission": payload.permission,
                "enabled": payload.enabled,
            }),
        }));

    Json(ApiEnvelope::success(
        "permission updated",
        json!({"permissions": updated_permissions}),
    ))
    .into_response()
}

/// Returns current runtime rate-limit configuration.
pub async fn get_rate_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = require_admin_user(&state, &headers).await {
        return response;
    }

    let config = state.rate_limit.get_config().await;
    Json(ApiEnvelope::success("rate config fetched", config)).into_response()
}

/// Updates runtime rate-limit configuration used by message/vote/dm flows.
pub async fn update_rate_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AdminUpdateRatePayload>,
) -> impl IntoResponse {
    let admin_user = match require_admin_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let mut config = state.rate_limit.get_config().await;

    if let Some(value) = payload.message_every_seconds {
        if value <= 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "message_every_seconds must be > 0",
                )),
            )
                .into_response();
        }
        config.message_every_seconds = value;
    }

    if let Some(value) = payload.upvote_every_seconds {
        if value <= 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "upvote_every_seconds must be > 0",
                )),
            )
                .into_response();
        }
        config.upvote_every_seconds = value;
    }

    if let Some(value) = payload.dm_per_minute {
        if value == 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "dm_per_minute must be > 0",
                )),
            )
                .into_response();
        }
        config.dm_per_minute = value;
    }

    if let Some(value) = payload.dm_per_hour {
        if value == 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "dm_per_hour must be > 0",
                )),
            )
                .into_response();
        }
        config.dm_per_hour = value;
    }

    if let Some(value) = payload.dm_per_day {
        if value == 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "dm_per_day must be > 0",
                )),
            )
                .into_response();
        }
        config.dm_per_day = value;
    }

    state.rate_limit.set_config(config.clone()).await;

    logging::log_activity(
        &state.db,
        "admin_rate_config_updated",
        Some(admin_user.id),
        json!({
            "message_every_seconds": config.message_every_seconds,
            "upvote_every_seconds": config.upvote_every_seconds,
            "dm_per_minute": config.dm_per_minute,
            "dm_per_hour": config.dm_per_hour,
            "dm_per_day": config.dm_per_day,
        }),
    )
    .await;

    Json(ApiEnvelope::success("rate config updated", config)).into_response()
}

/// Returns audit views for recent events and recent activity logs.
pub async fn audit_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AdminAuditQuery>,
) -> impl IntoResponse {
    if let Err(response) = require_admin_user(&state, &headers).await {
        return response;
    }

    let limit = query.limit.unwrap_or(50).clamp(1, 200);

    let payload = state
        .db
        .read(|db| {
            let events = db
                .events
                .iter()
                .rev()
                .take(limit)
                .cloned()
                .collect::<Vec<_>>();
            let logs = db
                .activity_logs
                .iter()
                .rev()
                .take(limit)
                .cloned()
                .collect::<Vec<_>>();

            AdminAuditPayload { events, logs }
        })
        .await;

    Json(ApiEnvelope::success("admin audit fetched", payload)).into_response()
}

/// Lists discussion edit requests pending moderation.
pub async fn list_edit_requests_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AdminDiscussionEditQuery>,
) -> impl IntoResponse {
    if let Err(response) = require_admin_user(&state, &headers).await {
        return response;
    }

    let status_filter = query
        .status
        .and_then(|value| DiscussionEditStatus::try_from(value).ok());

    let payload = state
        .db
        .read(|db| {
            let users = &db.users;
            let mut rows = db
                .discussion_edit_requests
                .iter()
                .filter(|req| status_filter.map_or(true, |status| req.status == status))
                .cloned()
                .collect::<Vec<_>>();

            rows.sort_by_key(|row| std::cmp::Reverse(row.requested_at));

            rows.into_iter()
                .map(|row| AdminDiscussionEditView {
                    request_id: row.id,
                    post_id: row.post_id,
                    author_id: row.author_id,
                    author_login: users
                        .get(&row.author_id)
                        .map(|user| user.login_username.clone()),
                    current_status: row.status as u8,
                    status_label: match row.status {
                        DiscussionEditStatus::Pending => "Pending",
                        DiscussionEditStatus::Approved => "Approved",
                        DiscussionEditStatus::Rejected => "Rejected",
                    }
                    .to_string(),
                    new_title: row.new_title.clone(),
                    new_body: row.new_body.clone(),
                    requested_at: row.requested_at,
                })
                .collect::<Vec<_>>()
        })
        .await;

    Json(ApiEnvelope::success(
        "discussion edit requests fetched",
        payload,
    ))
    .into_response()
}

/// Approves or rejects a pending discussion edit request.
pub async fn decide_edit_request_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AdminDiscussionEditDecisionPayload>,
) -> impl IntoResponse {
    let admin_user = match require_admin_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let now = Utc::now();
    let mut snapshot = None;
    let update_result = state
        .db
        .write(|db| {
            let request = match db
                .discussion_edit_requests
                .iter_mut()
                .find(|req| req.id == payload.request_id)
            {
                Some(value) => value,
                None => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(ApiEnvelope::<serde_json::Value>::error(
                            "edit request not found",
                        )),
                    )
                        .into_response());
                }
            };

            if request.status != DiscussionEditStatus::Pending {
                return Err((
                    StatusCode::CONFLICT,
                    Json(ApiEnvelope::<serde_json::Value>::error(
                        "request already resolved",
                    )),
                )
                    .into_response());
            }

            request.status = if payload.approve {
                DiscussionEditStatus::Approved
            } else {
                DiscussionEditStatus::Rejected
            };
            request.resolved_at = Some(now);
            request.resolved_by = Some(admin_user.id);
            snapshot = Some(request.clone());

            if payload.approve {
                if let Some(post) = db.discussion_posts.get_mut(&request.post_id) {
                    if let Some(title) = &request.new_title {
                        post.title = title.clone();
                    }
                    post.body = request.new_body.clone();
                    post.edited_at = Some(now);
                }
            }

            Ok(())
        })
        .await;

    if let Err(response) = update_result {
        return response;
    }

    let request_snapshot = snapshot.expect("request snapshot should exist");
    let event_type = if payload.approve {
        EventType::DiscussionEditApproved
    } else {
        EventType::DiscussionEditRejected
    };

    logging::log_activity(
        &state.db,
        "discussion_edit_resolved",
        Some(admin_user.id),
        json!({
            "request_id": request_snapshot.id,
            "status": request_snapshot.status as u8
        }),
    )
    .await;

    let event = state
        .events
        .insert_event_row(
            &state.db,
            event_type,
            json!({
                "request_id": request_snapshot.id,
                "post_id": request_snapshot.post_id,
                "resolved_by": admin_user.id,
                "status": request_snapshot.status as u8
            }),
        )
        .await;
    state.events.dispatch_after_commit(event);

    state
        .websocket_broadcaster
        .publish(WsServerEvent::AdminEvent(WsAdminEvent {
            action: if payload.approve {
                "discussion.edit.approved".to_string()
            } else {
                "discussion.edit.rejected".to_string()
            },
            details: json!({
                "request_id": request_snapshot.id,
                "status": request_snapshot.status as u8,
                "resolved_by": admin_user.id,
            }),
        }));

    Json(ApiEnvelope::success(
        "edit request processed",
        json!({"request_id": request_snapshot.id, "status": request_snapshot.status as u8}),
    ))
    .into_response()
}

/// Verifies cookie session and checks admin permission grants.
pub async fn require_admin_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::models::UserRecord, axum::response::Response> {
    // Authenticate user using signed cookie session.
    let Some(user) = auth::authenticated_user_from_headers(state, headers).await else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "admin authentication required",
            )),
        )
            .into_response());
    };

    // Require write-capable user status for admin actions.
    if !user.status.can_write() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "admin account must be active",
            )),
        )
            .into_response());
    }

    // Require explicit `suspend_user` permission for admin panel access.
    let can_admin = state
        .permissions
        .has(&state.db, user.id, permissions::SUSPEND_USER)
        .await;
    if !can_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "admin permission missing",
            )),
        )
            .into_response());
    }

    Ok(user)
}

/// Builds user view rows enriched with permission lists and active-session counters.
fn build_user_views(db: &crate::db::DatabaseState) -> Vec<AdminUserView> {
    let mut rows = db
        .users
        .values()
        .map(|user| {
            let mut permission_list = db
                .user_permissions
                .get(&user.id)
                .map(|set| set.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            permission_list.sort();

            let active_sessions = db
                .sessions
                .values()
                .filter(|session| session.user_id == user.id)
                .count();

            AdminUserView {
                id: user.id,
                login_username: user.login_username.clone(),
                public_username: user.public_username.clone(),
                status: user.status as u8,
                status_label: status_label(user.status).to_string(),
                created_at: user.created_at,
                updated_at: user.updated_at,
                permissions: permission_list,
                active_sessions,
            }
        })
        .collect::<Vec<_>>();

    // Sort newest-first for dashboard friendliness.
    rows.sort_by_key(|row| std::cmp::Reverse(row.created_at));
    rows
}

/// Returns static status label text for dashboard rendering.
fn status_label(status: UserStatus) -> &'static str {
    match status {
        UserStatus::Pending => "Pending",
        UserStatus::Active => "Active",
        UserStatus::Suspended => "Suspended",
        UserStatus::Banned => "Banned",
    }
}

/// Returns the allowed permission keys that can be toggled from admin panel.
fn allowed_permission_keys() -> &'static [&'static str] {
    &[
        permissions::DELETE_MESSAGE,
        permissions::SUSPEND_USER,
        permissions::APPROVE_DISCUSSION_EDIT,
        permissions::UPLOAD_FILE_LEVEL_1,
        permissions::UPLOAD_FILE_LEVEL_2,
        permissions::UPLOAD_FILE_LEVEL_3,
        permissions::MODERATE_GROUP,
        permissions::MODERATE_COMMUNITY,
    ]
}
