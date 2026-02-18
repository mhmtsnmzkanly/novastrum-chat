use axum::{Json, extract::State, http::HeaderMap};
use chrono::Utc;
use serde::Serialize;

use crate::{AppState, auth, models::ApiEnvelope};

/// Health response payload for monitoring endpoint.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Service status string.
    pub status: String,
    /// Current UTC timestamp.
    pub now_utc: String,
    /// Process uptime in seconds.
    pub uptime_seconds: u64,
    /// Number of active users currently online.
    pub online_user_count: usize,
}

/// Lightweight user summary used by app bootstrap endpoint.
#[derive(Debug, Serialize)]
pub struct BootstrapUser {
    /// User id.
    pub id: String,
    /// Login username.
    pub login_username: String,
    /// Human-readable status label.
    pub status: String,
}

/// Lightweight chat summary used by app bootstrap endpoint.
#[derive(Debug, Serialize)]
pub struct BootstrapChat {
    /// Chat id.
    pub id: String,
    /// Chat type label.
    pub chat_type: String,
    /// Creator id.
    pub created_by: String,
}

/// Chat membership row for bootstrap payload.
#[derive(Debug, Serialize)]
pub struct BootstrapChatMember {
    pub chat_id: String,
    pub user_id: String,
}

/// Friendship tuple.
#[derive(Debug, Serialize)]
pub struct BootstrapFriendship {
    pub user_a: String,
    pub user_b: String,
}

/// Pending DM request.
#[derive(Debug, Serialize)]
pub struct BootstrapDmRequest {
    pub id: String,
    pub from_user_id: String,
    pub to_user_id: String,
    pub created_at: String,
}

/// Bootstrap payload containing minimal IDs for frontend demo flows.
#[derive(Debug, Serialize)]
pub struct BootstrapState {
    /// Authenticated user id from signed session cookie, if available.
    pub current_user_id: Option<String>,
    /// Seeded users.
    pub users: Vec<BootstrapUser>,
    /// Seeded chats.
    pub chats: Vec<BootstrapChat>,
    /// Chat memberships (used for DM/group name resolution).
    pub chat_members: Vec<BootstrapChatMember>,
    /// Friendships.
    pub friendships: Vec<BootstrapFriendship>,
    /// Pending DM requests (incoming/outgoing).
    pub pending_dm_requests: Vec<BootstrapDmRequest>,
    /// Online user ids.
    pub online_user_ids: Vec<String>,
    /// Unread notification count for current user (if auth).
    pub unread_notifications: Option<u64>,
}

/// `/health` endpoint for uptime and lightweight runtime checks.
pub async fn health_handler(State(state): State<AppState>) -> Json<ApiEnvelope<HealthResponse>> {
    // Count users with at least one active WebSocket session.
    let users = state
        .db
        .read(|db| db.users.keys().copied().collect::<Vec<_>>())
        .await;

    let mut online = 0usize;
    for user_id in users {
        if state.presence.is_online(user_id).await {
            online = online.saturating_add(1);
        }
    }

    Json(ApiEnvelope::success(
        "health check ok",
        HealthResponse {
            status: "ok".to_string(),
            now_utc: Utc::now().to_rfc3339(),
            uptime_seconds: state.started_at.elapsed().as_secs(),
            online_user_count: online,
        },
    ))
}

/// Returns seed ids so `web/app.html` can call APIs without hard-coded UUIDs.
pub async fn bootstrap_state_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<ApiEnvelope<BootstrapState>> {
    // Resolve the authenticated session owner so frontend can pin selected user.
    let current_user_id = auth::authenticated_user_from_headers(&state, &headers)
        .await
        .map(|user| user.id.to_string());

    // Read users/chats once and map them into frontend-friendly JSON.
    let (mut users, mut chats, members, friendships, dm_reqs) = state
        .db
        .read(|db| {
            let users = db
                .users
                .values()
                .map(|user| BootstrapUser {
                    id: user.id.to_string(),
                    login_username: user.login_username.clone(),
                    status: format!("{:?}", user.status),
                })
                .collect::<Vec<_>>();

            let chats = db
                .chats
                .values()
                .map(|chat| BootstrapChat {
                    id: chat.id.to_string(),
                    chat_type: format!("{:?}", chat.chat_type),
                    created_by: chat.created_by.to_string(),
                })
                .collect::<Vec<_>>();

            let members = db
                .chat_members
                .iter()
                .map(|m| BootstrapChatMember {
                    chat_id: m.chat_id.to_string(),
                    user_id: m.user_id.to_string(),
                })
                .collect::<Vec<_>>();

            let friendships = db
                .friendships
                .iter()
                .map(|(a, b)| BootstrapFriendship {
                    user_a: a.to_string(),
                    user_b: b.to_string(),
                })
                .collect::<Vec<_>>();

            let dm_reqs = db
                .pending_dm_requests
                .iter()
                .map(|r| BootstrapDmRequest {
                    id: r.id.to_string(),
                    from_user_id: r.from_user_id.to_string(),
                    to_user_id: r.to_user_id.to_string(),
                    created_at: r.created_at.to_rfc3339(),
                })
                .collect::<Vec<_>>();

            (users, chats, members, friendships, dm_reqs)
        })
        .await;

    // Keep deterministic ordering so UI does not jump between users/chats on refresh.
    users.sort_by(|a, b| {
        a.login_username
            .cmp(&b.login_username)
            .then_with(|| a.id.cmp(&b.id))
    });
    chats.sort_by(|a, b| {
        chat_type_order(&a.chat_type)
            .cmp(&chat_type_order(&b.chat_type))
            .then_with(|| a.id.cmp(&b.id))
    });

    // presence online list
    let mut online_user_ids = Vec::new();
    for u in users.iter() {
        if let Ok(id) = uuid::Uuid::parse_str(&u.id) {
            if state.presence.is_online(id).await {
                online_user_ids.push(u.id.clone());
            }
        }
    }

    // unread for current user
    let unread_notifications = if let Some(user_id_str) = &current_user_id {
        if let Ok(id) = uuid::Uuid::parse_str(user_id_str) {
            Some(state.notifications.unread_count(id).await)
        } else {
            None
        }
    } else {
        None
    };

    Json(ApiEnvelope::success(
        "bootstrap state fetched",
        BootstrapState {
            current_user_id,
            users,
            chats,
            chat_members: members,
            friendships,
            pending_dm_requests: dm_reqs,
            online_user_ids,
            unread_notifications,
        },
    ))
}

/// Returns deterministic ordering key for chat types in bootstrap payload.
fn chat_type_order(chat_type: &str) -> u8 {
    match chat_type {
        "Community" => 0,
        "Group" => 1,
        "DirectMessage" => 2,
        _ => 3,
    }
}
