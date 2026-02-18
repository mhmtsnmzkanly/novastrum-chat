use std::{collections::HashMap, sync::Arc};

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    AppState, auth,
    events::EventType,
    logging,
    models::{ApiEnvelope, CursorPage, NotificationItem},
    websocket::{WsNotification, WsNotificationUnread, WsServerEvent},
};

/// Mute settings for one `(user, chat)` pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMute {
    /// User id owning the mute setting.
    pub user_id: Uuid,
    /// Chat id for muted notifications.
    pub chat_id: Uuid,
    /// Optional expiration for the mute window.
    pub mute_until: Option<DateTime<Utc>>,
}

/// Internal mutable notification state.
#[derive(Debug, Default)]
struct NotificationState {
    /// Per user unread counters.
    unread_counters: HashMap<Uuid, u64>,
    /// Chat mute table keyed by `(user_id, chat_id)`.
    chat_mutes: HashMap<(Uuid, Uuid), ChatMute>,
}

/// Notification service handling mute logic and unread counters.
#[derive(Debug, Clone)]
pub struct NotificationService {
    /// Lock-protected in-memory notification state.
    state: Arc<RwLock<NotificationState>>,
}

impl NotificationService {
    /// Creates an empty notification service.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(NotificationState::default())),
        }
    }

    /// Updates mute settings for a chat.
    pub async fn set_chat_mute(
        &self,
        user_id: Uuid,
        chat_id: Uuid,
        mute_until: Option<DateTime<Utc>>,
    ) {
        let mut guard = self.state.write().await;
        guard.chat_mutes.insert(
            (user_id, chat_id),
            ChatMute {
                user_id,
                chat_id,
                mute_until,
            },
        );
    }

    /// Returns true when notification should be delivered.
    pub async fn should_deliver(
        &self,
        user_id: Uuid,
        chat_id: Option<Uuid>,
        mention_override: bool,
        now: DateTime<Utc>,
    ) -> bool {
        // Mentions always bypass mute according to specification.
        if mention_override {
            return true;
        }

        let Some(chat_id) = chat_id else {
            return true;
        };

        let guard = self.state.read().await;
        let Some(mute) = guard.chat_mutes.get(&(user_id, chat_id)) else {
            return true;
        };

        match mute.mute_until {
            Some(until) => now > until,
            None => false,
        }
    }

    /// Inserts notification row and updates unread counter.
    pub async fn create_notification(
        &self,
        state: &AppState,
        user_id: Uuid,
        chat_id: Option<Uuid>,
        kind: &str,
        body: &str,
        mention_override: bool,
    ) -> Option<NotificationItem> {
        let now = Utc::now();

        // Skip insertion when mute is active and no mention override is present.
        if !self
            .should_deliver(user_id, chat_id, mention_override, now)
            .await
        {
            return None;
        }

        // Build immutable row before persisting to database.
        let row = NotificationItem {
            id: Uuid::new_v4(),
            user_id,
            chat_id,
            kind: kind.to_string(),
            body: body.to_string(),
            unread: true,
            created_at: now,
        };

        // Persist notification row in the shared DB table.
        state
            .db
            .write(|db| {
                db.notifications.push(row.clone());
            })
            .await;

        // Update unread counter in mutable notification state.
        {
            let mut guard = self.state.write().await;
            let counter = guard.unread_counters.entry(user_id).or_insert(0);
            *counter = counter.saturating_add(1);
        }

        // Emit notification-created event and dispatch asynchronously.
        let event = state
            .events
            .insert_event_row(
                &state.db,
                EventType::NotificationCreated,
                json!({
                    "notification_id": row.id,
                    "user_id": row.user_id,
                    "kind": row.kind,
                    "chat_id": row.chat_id,
                }),
            )
            .await;
        state.events.dispatch_after_commit(event);

        // Add activity log entry for auditability.
        logging::log_activity(
            &state.db,
            "notification_created",
            Some(user_id),
            json!({"notification_id": row.id, "kind": row.kind}),
        )
        .await;

        let unread_now = self.unread_count(user_id).await;

        state
            .websocket_broadcaster
            .publish(WsServerEvent::Notification(WsNotification {
                unread_count: Some(unread_now),
                ..WsNotification::from(&row)
            }));

        state
            .websocket_broadcaster
            .publish(WsServerEvent::NotificationUnread(WsNotificationUnread {
                user_id,
                unread: unread_now,
            }));

        Some(row)
    }

    /// Returns unread counter for one user.
    pub async fn unread_count(&self, user_id: Uuid) -> u64 {
        let guard = self.state.read().await;
        guard.unread_counters.get(&user_id).copied().unwrap_or(0)
    }
}

/// Returns unread counter for the authenticated user.
pub async fn unread_count_handler(
    State(state): State<AppState>,
    Query(query): Query<ListNotificationQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user = match auth::require_authenticated_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response.into_response(),
    };
    if user.id != query.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error("session user mismatch")),
        )
            .into_response();
    }

    let unread = state.notifications.unread_count(user.id).await;
    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "unread count",
            json!({ "unread": unread }),
        )),
    )
        .into_response()
}

/// Query params for listing user notifications.
#[derive(Debug, Deserialize)]
pub struct ListNotificationQuery {
    /// User id whose notifications should be listed.
    pub user_id: Uuid,
    /// Optional unread-only filter.
    pub unread_only: Option<bool>,
    /// Cursor for pagination (ISO timestamp filter).
    pub cursor: Option<String>,
    /// Optional limit per page.
    pub limit: Option<usize>,
}

/// HTTP handler for reading notifications.
pub async fn list_notifications_handler(
    State(state): State<AppState>,
    Query(query): Query<ListNotificationQuery>,
    headers: HeaderMap,
) -> Response {
    let user = match auth::require_authenticated_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response.into_response(),
    };

    if user.id != query.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "session user mismatch",
            )),
        )
            .into_response();
    }

    let unread_only = query.unread_only.unwrap_or(false);

    // Load notifications from DB and apply user/unread filters.
    let cursor_threshold = query
        .cursor
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let limit = query.limit.unwrap_or(50).clamp(10, 200);

    let (rows, next_cursor) = state
        .db
        .read(|db| {
            let mut items = db
                .notifications
                .iter()
                .filter(|item| item.user_id == user.id)
                .filter(|item| !unread_only || item.unread)
                .cloned()
                .collect::<Vec<_>>();

            items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            if let Some(cursor) = cursor_threshold {
                items.retain(|item| item.created_at < cursor);
            }

            let limited = items.into_iter().take(limit).collect::<Vec<_>>();
            let next = limited.last().map(|entry| entry.created_at.to_rfc3339());

            (limited, next)
        })
        .await;

    Json(ApiEnvelope::success(
        "notifications fetched",
        CursorPage {
            items: rows,
            next_cursor,
        },
    ))
    .into_response()
}
