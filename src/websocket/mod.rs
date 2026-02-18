use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json, to_string};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    AppState, auth,
    events::EventType,
    logging,
    models::{ChatMessage, ChatType, DiscussionPost, NotificationItem},
};

/// Server-originated websocket event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsServerEvent {
    #[serde(rename = "chat.message")]
    ChatMessage(WsChatMessage),
    #[serde(rename = "discussion.post")]
    DiscussionPost(WsDiscussionPost),
    #[serde(rename = "discussion.vote")]
    DiscussionVote(WsDiscussionVote),
    #[serde(rename = "notification.created")]
    Notification(WsNotification),
    #[serde(rename = "notification.unread")]
    NotificationUnread(WsNotificationUnread),
    #[serde(rename = "admin.event")]
    AdminEvent(WsAdminEvent),
    #[serde(rename = "presence.state")]
    Presence(WsPresence),
}

/// Payload sent when a new chat message is stored.
#[derive(Debug, Clone, Serialize)]
pub struct WsChatMessage {
    pub chat_id: Uuid,
    pub chat_type: ChatType,
    pub message: WsMessageDto,
}

/// Simplified chat message representation used for websocket pushes.
#[derive(Debug, Clone, Serialize)]
pub struct WsMessageDto {
    pub id: Uuid,
    pub body: String,
    pub sender_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted: bool,
}

impl From<&ChatMessage> for WsMessageDto {
    fn from(message: &ChatMessage) -> Self {
        Self {
            id: message.id,
            body: message.body.clone(),
            sender_id: message.sender_id,
            created_at: message.created_at,
            edited_at: message.edited_at,
            deleted: message.deleted,
        }
    }
}

/// Payload for discussion post broadcasts.
#[derive(Debug, Clone, Serialize)]
pub struct WsDiscussionPost {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub depth: u8,
    pub author_id: Uuid,
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

impl From<&DiscussionPost> for WsDiscussionPost {
    fn from(post: &DiscussionPost) -> Self {
        Self {
            id: post.id,
            thread_id: post.thread_id,
            parent_id: post.parent_id,
            depth: post.depth,
            author_id: post.author_id,
            title: post.title.clone(),
            body: post.body.clone(),
            created_at: post.created_at,
        }
    }
}

/// Payload delivered when a discussion vote changes.
#[derive(Debug, Clone, Serialize)]
pub struct WsDiscussionVote {
    pub post_id: Uuid,
    pub score: i64,
}

/// Notification broadcast payload.
#[derive(Debug, Clone, Serialize)]
pub struct WsNotification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub chat_id: Option<Uuid>,
    pub kind: String,
    pub body: String,
    pub unread: bool,
    pub unread_count: Option<u64>,
    pub created_at: DateTime<Utc>,
}

impl From<&NotificationItem> for WsNotification {
    fn from(item: &NotificationItem) -> Self {
        Self {
            id: item.id,
            user_id: item.user_id,
            chat_id: item.chat_id,
            kind: item.kind.clone(),
            body: item.body.clone(),
            unread: item.unread,
            unread_count: None,
            created_at: item.created_at,
        }
    }
}

/// Unread counter push payload.
#[derive(Debug, Clone, Serialize)]
pub struct WsNotificationUnread {
    pub user_id: Uuid,
    pub unread: u64,
}

/// Administrative push payload (user actions, rate config changes, etc.).
#[derive(Debug, Clone, Serialize)]
pub struct WsAdminEvent {
    pub action: String,
    pub details: Value,
}

/// Presence payload.
#[derive(Debug, Clone, Serialize)]
pub struct WsPresence {
    pub user_id: Uuid,
    pub online: bool,
    pub active_sessions: usize,
}

/// Broadcast hub that fans out server events to every connected websocket.
#[derive(Clone)]
pub struct WebSocketBroadcaster {
    sender: broadcast::Sender<WsServerEvent>,
}

impl WebSocketBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(128);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsServerEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: WsServerEvent) {
        let _ = self.sender.send(event);
    }
}

/// Upgrades HTTP connection to WebSocket after session validation.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let session = match auth::require_authenticated_session(&state, &headers).await {
        Ok(session) => session,
        Err(response) => return response.into_response(),
    };

    let user_id = session.user_id;
    let session_id = session.session_id.clone();

    ws.on_upgrade(move |socket| handle_socket(state, socket, user_id, session_id))
}

/// Runs websocket lifecycle: connect -> activity tracking -> disconnect.
async fn handle_socket(state: AppState, mut socket: WebSocket, user_id: Uuid, session_id: String) {
    let connection_id = Uuid::new_v4();
    let now = Utc::now();

    let mut event_rx = state.websocket_broadcaster.subscribe();

    // Record active presence session.
    state.presence.connect(user_id, connection_id, now).await;
    let session_count = state.presence.active_session_count(user_id).await;
    state.websocket_broadcaster.publish(WsServerEvent::Presence(WsPresence {
        user_id,
        online: true,
        active_sessions: session_count,
    }));

    // Store user connected event.
    let connected_event = state
        .events
        .insert_event_row(
            &state.db,
            EventType::UserConnected,
            json!({
                "user_id": user_id,
                "connection_id": connection_id,
                "session_id": session_id,
            }),
        )
        .await;
    state.events.dispatch_after_commit(connected_event);

    // Write activity log for connection start.
    logging::log_activity(
        &state.db,
        "user_ws_connected",
        Some(user_id),
        json!({"connection_id": connection_id}),
    )
    .await;

    // Process frames and broadcasts until disconnect.
    loop {
        tokio::select! {
            result = event_rx.recv() => {
                match result {
                    Ok(event) => {
                        let payload = match to_string(&event) {
                            Ok(serialized) => serialized,
                            Err(error) => {
                                tracing::warn!(error = %error, "failed to serialize websocket event");
                                continue;
                            }
                        };
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Some(Ok(_message)) => {
                        let touch_time = Utc::now();
                        state.presence.touch(connection_id, touch_time).await;
                        state
                            .db
                            .write(|db| {
                                if let Some(session) = db.sessions.get_mut(&session_id) {
                                    session.last_activity_at = touch_time;
                                }
                            })
                            .await;
                    }
                    Some(Err(error)) => {
                        tracing::warn!(error = %error, "websocket receive error");
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    // Mark presence as disconnected when socket loop ends.
    let disconnected_at = Utc::now();
    state
        .presence
        .disconnect(connection_id, disconnected_at)
        .await;

    // Broadcast presence update.
    let remaining = state.presence.active_session_count(user_id).await;
    state
        .websocket_broadcaster
        .publish(WsServerEvent::Presence(WsPresence {
            user_id,
            online: remaining > 0,
            active_sessions: remaining,
        }));

    // Store user disconnected event.
    let disconnected_event = state
        .events
        .insert_event_row(
            &state.db,
            EventType::UserDisconnected,
            json!({
                "user_id": user_id,
                "connection_id": connection_id,
                "session_id": session_id,
            }),
        )
        .await;
    state.events.dispatch_after_commit(disconnected_event);

    // Write activity log for connection end.
    logging::log_activity(
        &state.db,
        "user_ws_disconnected",
        Some(user_id),
        json!({"connection_id": connection_id}),
    )
    .await;
}
