use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use axum::{Json, extract::{Query, State}, http::{HeaderMap, StatusCode}, response::IntoResponse};
use serde_json::json;

use crate::{AppState, auth, models::ApiEnvelope};

/// Presence row representing one active WebSocket connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceSession {
    /// Unique connection id generated for each WebSocket connect.
    pub connection_id: Uuid,
    /// Session owner user id.
    pub user_id: Uuid,
    /// Connection start timestamp.
    pub connected_at: DateTime<Utc>,
    /// Last seen timestamp updated by ping/messages.
    pub last_seen: DateTime<Utc>,
    /// Disconnect timestamp set when connection closes.
    pub disconnected_at: Option<DateTime<Utc>>,
}

/// Mutable in-memory presence state.
#[derive(Debug, Default)]
struct PresenceState {
    /// Active sessions keyed by connection id.
    sessions: HashMap<Uuid, PresenceSession>,
}

/// Query params for presence lookup.
#[derive(Debug, Deserialize)]
pub struct PresenceQuery {
    /// Optional list of user ids; if omitted returns all online users.
    pub user_ids: Option<Vec<Uuid>>,
}


/// Tracks active sessions and online status (`active_session_count > 0`).
#[derive(Debug, Clone)]
pub struct PresenceService {
    /// Lock-protected presence state.
    state: Arc<RwLock<PresenceState>>,
}

impl PresenceService {
    /// Creates an empty presence tracker.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(PresenceState::default())),
        }
    }

    /// Inserts a new active presence session on WebSocket connect.
    pub async fn connect(&self, user_id: Uuid, connection_id: Uuid, now: DateTime<Utc>) {
        let mut guard = self.state.write().await;
        guard.sessions.insert(
            connection_id,
            PresenceSession {
                connection_id,
                user_id,
                connected_at: now,
                last_seen: now,
                disconnected_at: None,
            },
        );
    }

    /// Marks session disconnected and returns owning user id when found.
    pub async fn disconnect(&self, connection_id: Uuid, now: DateTime<Utc>) -> Option<Uuid> {
        let mut guard = self.state.write().await;
        let session = guard.sessions.get_mut(&connection_id)?;
        session.disconnected_at = Some(now);
        Some(session.user_id)
    }

    /// Updates `last_seen` for an active connection.
    pub async fn touch(&self, connection_id: Uuid, now: DateTime<Utc>) {
        let mut guard = self.state.write().await;
        if let Some(session) = guard.sessions.get_mut(&connection_id)
            && session.disconnected_at.is_none()
        {
            session.last_seen = now;
        }
    }

    /// Returns active session count for a user.
    pub async fn active_session_count(&self, user_id: Uuid) -> usize {
        let guard = self.state.read().await;
        guard
            .sessions
            .values()
            .filter(|session| session.user_id == user_id && session.disconnected_at.is_none())
            .count()
    }

    /// Returns whether user is online according to `active_session_count > 0`.
    pub async fn is_online(&self, user_id: Uuid) -> bool {
        self.active_session_count(user_id).await > 0
    }
}

/// Returns online users (filtered by optional list) for authenticated sessions.
pub async fn presence_status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PresenceQuery>,
) -> impl IntoResponse {
    if let Err(response) = auth::require_authenticated_user(&state, &headers).await {
        return response;
    }

    let candidates: Vec<Uuid> = if let Some(ids) = query.user_ids.clone() {
        ids
    } else {
        state
            .db
            .read(|db| db.users.keys().copied().collect::<Vec<_>>())
            .await
    };

    let mut online = Vec::new();
    for uid in candidates {
        if state.presence.is_online(uid).await {
            online.push(uid);
        }
    }

    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "presence listed",
            json!({"online_user_ids": online}),
        )),
    )
}
