use std::collections::HashSet;

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    AppState, auth,
    db::{DatabaseState, DmRequestRow, normalize_friend_pair},
    events::EventType,
    logging,
    models::{
        ApiEnvelope, ChatMember, ChatMessage, ChatRoom, ChatType, CursorPage, UserStatus,
        normalize_username,
    },
    websocket::{WsAdminEvent, WsChatMessage, WsMessageDto, WsServerEvent},
};

/// Payload for sending chat message.
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    /// Sender user id.
    pub user_id: Uuid,
    /// Target chat id.
    pub chat_id: Uuid,
    /// Chat type routing key.
    pub chat_type: ChatType,
    /// Raw text body.
    pub body: String,
}

/// Payload for requesting a DM with another user.
#[derive(Debug, Deserialize)]
pub struct RequestDmPayload {
    /// Sender user id.
    pub from_user_id: Uuid,
    /// Recipient user id.
    pub to_user_id: Uuid,
}

/// Payload for creating a new group chat.
#[derive(Debug, Deserialize)]
pub struct CreateGroupPayload {
    /// Creator user id.
    pub user_id: Uuid,
    /// Additional member ids (must include at least two).
    pub member_ids: Vec<Uuid>,
}

/// Payload for editing an existing message.
#[derive(Debug, Deserialize)]
pub struct EditMessagePayload {
    /// Editor user id (must be the sender).
    pub user_id: Uuid,
    /// Target message id.
    pub message_id: Uuid,
    /// Chat type routing key.
    pub chat_type: ChatType,
    /// New message body.
    pub new_body: String,
}

/// Payload for soft-deleting a message.
#[derive(Debug, Deserialize)]
pub struct DeleteMessagePayload {
    /// Requesting user id (must be the sender).
    pub user_id: Uuid,
    /// Target message id.
    pub message_id: Uuid,
    /// Chat type routing key.
    pub chat_type: ChatType,
}

/// Query params for listing chat history.
#[derive(Debug, Deserialize)]
pub struct ListMessagesQuery {
    pub chat_id: Uuid,
    pub chat_type: ChatType,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

/// Payload for accepting or rejecting a DM request.
#[derive(Debug, Deserialize)]
pub struct DecideDmRequestPayload {
    /// Recipient user id (must match request target).
    pub user_id: Uuid,
    /// Pending request id.
    pub request_id: Uuid,
}

/// Payload for group rename.
#[derive(Debug, Deserialize)]
pub struct RenameGroupPayload {
    pub user_id: Uuid,
    pub chat_id: Uuid,
    pub name: String,
}

/// Payload for group membership change.
#[derive(Debug, Deserialize)]
pub struct GroupMemberChangePayload {
    pub user_id: Uuid,
    pub chat_id: Uuid,
    pub target_user_id: Uuid,
}

/// Payload for deleting a group chat.
#[derive(Debug, Deserialize)]
pub struct DeleteGroupPayload {
    /// Group creator user id.
    pub user_id: Uuid,
    /// Target group chat id.
    pub chat_id: Uuid,
}

/// Renames a group chat (creator only).
pub async fn rename_group_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RenameGroupPayload>,
) -> impl IntoResponse {
    let _user = match auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let updated = state
        .db
        .write(|db| {
            if let Some(chat) = db.chats.get_mut(&payload.chat_id) {
                if chat.chat_type != ChatType::Group || chat.created_by != payload.user_id {
                    return false;
                }
                chat.label = Some(payload.name.trim().to_string());
                return true;
            }
            false
        })
        .await;

    if !updated {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "group not found or user not creator",
            )),
        );
    }

    (
        StatusCode::OK,
        Json(ApiEnvelope::<serde_json::Value>::success_no_data("group renamed")),
    )
}

/// Adds a member to a group (creator only).
pub async fn add_group_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GroupMemberChangePayload>,
) -> impl IntoResponse {
    let _user = match auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let added = state
        .db
        .write(|db| {
            let Some(chat) = db.chats.get(&payload.chat_id) else { return false; };
            if chat.chat_type != ChatType::Group || chat.created_by != payload.user_id {
                return false;
            }
            if !db.users.contains_key(&payload.target_user_id) {
                return false;
            }
            let exists = db.chat_members.iter().any(|m| m.chat_id == payload.chat_id && m.user_id == payload.target_user_id);
            if exists {
                return true;
            }
            db.chat_members.push(ChatMember {
                chat_id: payload.chat_id,
                user_id: payload.target_user_id,
                joined_at: Utc::now(),
            });
            true
        })
        .await;

    if !added {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "cannot add member",
            )),
        );
    }

    (StatusCode::OK, Json(ApiEnvelope::<serde_json::Value>::success_no_data("member added")))
}

/// Removes a member from a group (creator only).
pub async fn remove_group_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GroupMemberChangePayload>,
) -> impl IntoResponse {
    let _user = match auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let removed = state
        .db
        .write(|db| {
            let Some(chat) = db.chats.get(&payload.chat_id) else { return false; };
            if chat.chat_type != ChatType::Group || chat.created_by != payload.user_id {
                return false;
            }
            let before = db.chat_members.len();
            db.chat_members.retain(|m| !(m.chat_id == payload.chat_id && m.user_id == payload.target_user_id));
            before != db.chat_members.len()
        })
        .await;

    if !removed {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "cannot remove member",
            )),
        );
    }

    (StatusCode::OK, Json(ApiEnvelope::<serde_json::Value>::success_no_data("member removed")))
}

/// Deletes a group chat (creator only) and prunes its membership/messages.
pub async fn delete_group_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DeleteGroupPayload>,
) -> impl IntoResponse {
    let _user = match auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let deleted = state
        .db
        .write(|db| {
            let Some(chat) = db.chats.get(&payload.chat_id) else { return false; };
            if chat.chat_type != ChatType::Group || chat.created_by != payload.user_id {
                return false;
            }
            // remove chat row
            db.chats.remove(&payload.chat_id);
            // remove memberships
            db.chat_members.retain(|m| m.chat_id != payload.chat_id);
            // remove group messages
            db.group_messages.retain(|m| m.chat_id != payload.chat_id);
            true
        })
        .await;

    if !deleted {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error("cannot delete group")),
        );
    }

    logging::log_activity(
        &state.db,
        "group_deleted",
        Some(payload.user_id),
        json!({"chat_id": payload.chat_id}),
    )
    .await;

    state.websocket_broadcaster.publish(WsServerEvent::AdminEvent(WsAdminEvent {
        action: "group.deleted".to_string(),
        details: json!({"chat_id": payload.chat_id, "by": payload.user_id}),
    }));

    (
        StatusCode::OK,
        Json(ApiEnvelope::<serde_json::Value>::success_no_data("group deleted")),
    )
}

/// Lists pending DM/friend requests for the authenticated user.
pub async fn list_dm_requests_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user = match auth::require_authenticated_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response.into_response(),
    };

    let requests: Vec<DmRequestRow> = state
        .db
        .read(|db| {
            db.pending_dm_requests
                .iter()
                .filter(|row| row.to_user_id == user.id)
                .cloned()
                .collect()
        })
        .await;

    (
        StatusCode::OK,
        Json(ApiEnvelope::success("dm requests listed", json!(requests))),
    )
        .into_response()
}

/// Accepts a pending DM/friend request: creates friendship + DM chat, removes request.
pub async fn accept_dm_request_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DecideDmRequestPayload>,
) -> impl IntoResponse {
    let user = match auth::require_authenticated_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response.into_response(),
    };
    if user.id != payload.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error("session user mismatch")),
        )
            .into_response();
    }

    // Remove pending request and retrieve it.
    let request_row = state
        .db
        .write(|db| {
            if let Some(pos) = db
                .pending_dm_requests
                .iter()
                .position(|r| r.id == payload.request_id && r.to_user_id == user.id)
            {
                Some(db.pending_dm_requests.remove(pos))
            } else {
                None
            }
        })
        .await;

    let Some(request_row) = request_row else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("pending request not found")),
        )
            .into_response();
    };

    // Create friendship link.
    let pair_key = normalize_friend_pair(request_row.from_user_id, request_row.to_user_id);
    state
        .db
        .write(|db| {
            db.friendships.insert(pair_key);
        })
        .await;

    // Create DM chat.
    let chat = ChatRoom {
        id: Uuid::new_v4(),
        chat_type: ChatType::DirectMessage,
        created_by: request_row.from_user_id,
        created_at: Utc::now(),
        label: None,
    };

    state
        .db
        .write(|db| {
            db.chats.insert(chat.id, chat.clone());
            db.chat_members.push(ChatMember {
                chat_id: chat.id,
                user_id: request_row.from_user_id,
                joined_at: Utc::now(),
            });
            db.chat_members.push(ChatMember {
                chat_id: chat.id,
                user_id: request_row.to_user_id,
                joined_at: Utc::now(),
            });
        })
        .await;

    let event = state
        .events
        .insert_event_row(
            &state.db,
            EventType::FriendAccepted,
            json!({
                "from_user_id": request_row.from_user_id,
                "to_user_id": request_row.to_user_id,
                "chat_id": chat.id,
            }),
        )
        .await;
    state.events.dispatch_after_commit(event);

    logging::log_activity(
        &state.db,
        "dm_request_accepted",
        Some(user.id),
        json!({"request_id": request_row.id, "chat_id": chat.id}),
    )
    .await;

    // Broadcast admin event so UIs can refresh without polling.
    state
        .websocket_broadcaster
        .publish(WsServerEvent::AdminEvent(WsAdminEvent {
            action: "dm.request.accepted".to_string(),
            details: json!({
                "request_id": request_row.id,
                "from_user_id": request_row.from_user_id,
                "to_user_id": request_row.to_user_id,
                "chat_id": chat.id,
            }),
        }));

    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "dm request accepted",
            json!({ "chat_id": chat.id }),
        )),
    )
        .into_response()
}

/// Rejects a pending DM/friend request.
pub async fn reject_dm_request_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DecideDmRequestPayload>,
) -> impl IntoResponse {
    let user = match auth::require_authenticated_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response.into_response(),
    };
    if user.id != payload.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error("session user mismatch")),
        )
            .into_response();
    }

    let removed = state
        .db
        .write(|db| {
            if let Some(pos) = db
                .pending_dm_requests
                .iter()
                .position(|r| r.id == payload.request_id && r.to_user_id == user.id)
            {
                db.pending_dm_requests.remove(pos);
                true
            } else {
                false
            }
        })
        .await;

    if !removed {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("pending request not found")),
        )
            .into_response();
    }

    logging::log_activity(
        &state.db,
        "dm_request_rejected",
        Some(user.id),
        json!({"request_id": payload.request_id}),
    )
    .await;

    state
        .websocket_broadcaster
        .publish(WsServerEvent::AdminEvent(WsAdminEvent {
            action: "dm.request.rejected".to_string(),
            details: json!({
                "request_id": payload.request_id,
                "user_id": user.id,
            }),
        }));

    (
        StatusCode::OK,
        Json(ApiEnvelope::<serde_json::Value>::success_no_data(
            "dm request rejected",
        )),
    )
        .into_response()
}

/// Lists messages for a chat with cursor-based pagination.
pub async fn list_messages_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListMessagesQuery>,
) -> impl IntoResponse {
    let user = match auth::require_authenticated_user(&state, &headers).await {
        Ok(user) => user,
        Err(response) => return response.into_response(),
    };

    if query.chat_type != ChatType::Community {
        let is_member = state
            .db
            .read(|db| {
                db.chat_members
                    .iter()
                    .any(|m| m.chat_id == query.chat_id && m.user_id == user.id)
            })
            .await;
        if !is_member {
            return (
                StatusCode::FORBIDDEN,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "user is not member of this chat",
                )),
            )
                .into_response();
        }
    }

    let cursor_dt = query
        .cursor
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let limit = query.limit.unwrap_or(50).clamp(10, 200);

    let items = state
        .db
        .read(|db| {
            let mut collected = Vec::with_capacity(limit);
            let mut iter: Vec<&ChatMessage> = match query.chat_type {
                ChatType::DirectMessage => db.dm_messages.iter().filter(|m| m.chat_id == query.chat_id).collect(),
                ChatType::Group => db.group_messages.iter().filter(|m| m.chat_id == query.chat_id).collect(),
                ChatType::Community => db.community_messages.iter().filter(|m| m.chat_id == query.chat_id).collect(),
            };
            iter.sort_by_key(|m| m.created_at);
            for msg in iter.into_iter().rev() {
                if let Some(c) = cursor_dt {
                    if msg.created_at >= c { continue; }
                }
                collected.push(msg.clone());
                if collected.len() >= limit { break; }
            }
            collected.reverse();
            collected
        })
        .await;
    let next_cursor = items.first().map(|m| m.created_at.to_rfc3339());
    let page = CursorPage { items, next_cursor };

    (
        StatusCode::OK,
        Json(ApiEnvelope::success("messages listed", page)),
    )
        .into_response()
}

/// Sends a message by executing the transaction boundary steps from the spec.
pub async fn send_message_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SendMessageRequest>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }
    // BEGIN TRANSACTION LOGIC

    // Step 1: status check.
    let sender = state
        .db
        .read(|db| db.users.get(&payload.user_id).cloned())
        .await;
    let Some(sender) = sender else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("sender not found")),
        );
    };

    if sender.status == UserStatus::Banned || sender.status == UserStatus::Pending {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "account status cannot send messages",
            )),
        );
    }

    if !sender.status.can_write() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "suspended users cannot write",
            )),
        );
    }

    // Step 2: permission check (chat existence + membership constraints).
    let chat_row = state
        .db
        .read(|db| db.chats.get(&payload.chat_id).cloned())
        .await;
    let Some(chat_row) = chat_row else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("chat not found")),
        );
    };

    if chat_row.chat_type != payload.chat_type {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "chat type mismatch for target chat",
            )),
        );
    }

    if payload.chat_type != ChatType::Community {
        let is_member = state
            .db
            .read(|db| {
                db.chat_members.iter().any(|member| {
                    member.chat_id == payload.chat_id && member.user_id == payload.user_id
                })
            })
            .await;
        if !is_member {
            return (
                StatusCode::FORBIDDEN,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "user is not member of this chat",
                )),
            );
        }
    }

    // Step 3: rate limit check (`1 message / 3 sec`).
    let now = Utc::now();
    if !state.rate_limit.allow_message(payload.user_id, now).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "message rate limit exceeded",
            )),
        );
    }

    // Basic content validation.
    if payload.body.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "message body cannot be empty",
            )),
        );
    }

    // Step 5 (prepared before writes): mention parsing with max=5 and exact username match.
    let mentions = parse_mentions(&state, &payload.body).await;
    if mentions.len() > 5 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "maximum 5 mentions allowed per message",
            )),
        );
    }

    // Step 4: insert message into type-specific table.
    let message = ChatMessage {
        id: Uuid::new_v4(),
        chat_id: payload.chat_id,
        chat_type: payload.chat_type,
        sender_id: payload.user_id,
        body: payload.body.trim().to_string(),
        mentions: mentions.clone(),
        deleted: false,
        created_at: now,
        edited_at: None,
    };

    state
        .db
        .write(|db| match payload.chat_type {
            ChatType::DirectMessage => db.dm_messages.push(message.clone()),
            ChatType::Group => db.group_messages.push(message.clone()),
            ChatType::Community => db.community_messages.push(message.clone()),
        })
        .await;

    // Step 6: insert mention notifications (mention overrides mute).
    for mentioned_user in &mentions {
        let _ = state
            .notifications
            .create_notification(
                &state,
                *mentioned_user,
                Some(payload.chat_id),
                "mention",
                "You were mentioned in a message",
                true,
            )
            .await;
    }

    // Step 7: insert event row inside transaction semantics.
    let event_row = state
        .events
        .insert_event_row(
            &state.db,
            EventType::MessageSent,
            json!({
                "message_id": message.id,
                "chat_id": message.chat_id,
                "chat_type": message.chat_type as u8,
                "sender_id": message.sender_id,
                "mentions": message.mentions,
            }),
        )
        .await;

    // Step 8: insert system/activity log row.
    logging::log_activity(
        &state.db,
        "message_sent",
        Some(payload.user_id),
        json!({"message_id": message.id, "chat_id": message.chat_id}),
    )
    .await;

    // COMMIT completed; dispatch async side effects afterward.
    state.events.dispatch_after_commit(event_row);

    state
        .websocket_broadcaster
        .publish(WsServerEvent::ChatMessage(WsChatMessage {
            chat_id: payload.chat_id,
            chat_type: payload.chat_type,
            message: WsMessageDto::from(&message),
        }));

    (
        StatusCode::OK,
        Json(ApiEnvelope::success("message sent", json!(message))),
    )
}

/// Edits a message within the 300 second window after creation.
pub async fn edit_message_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<EditMessagePayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }

    let message = state
        .db
        .read(|db| find_message(db, payload.chat_type, payload.message_id).cloned())
        .await;
    let Some(message) = message else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("message not found")),
        );
    };

    if message.sender_id != payload.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "only the sender can edit",
            )),
        );
    }

    if message.deleted {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error("message already deleted")),
        );
    }

    let now = Utc::now();
    if now - message.created_at > Duration::seconds(300) {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "edit window expired (300 seconds)",
            )),
        );
    }

    // Ensure author still has access to the chat (for non-community types).
    if !has_chat_access(&state, payload.chat_type, message.chat_id, payload.user_id).await {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error("user is not member of this chat")),
        );
    }

    let new_body = payload.new_body.trim();
    if new_body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error("message body cannot be empty")),
        );
    }

    let mentions = parse_mentions(&state, new_body).await;

    // Apply mutation.
    let updated_message = state
        .db
        .write(|db| {
            let message = find_message_mut(db, payload.chat_type, payload.message_id)?;
            message.body = new_body.to_string();
            message.mentions = mentions.clone();
            message.edited_at = Some(now);
            Some(message.clone())
        })
        .await;

    let Some(updated_message) = updated_message else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("message not found")),
        );
    };

    let event_row = state
        .events
        .insert_event_row(
            &state.db,
            EventType::MessageEdited,
            json!({
                "message_id": updated_message.id,
                "chat_id": updated_message.chat_id,
                "chat_type": updated_message.chat_type as u8,
                "editor_id": payload.user_id,
                "mentions": updated_message.mentions,
            }),
        )
        .await;
    state.events.dispatch_after_commit(event_row);

    logging::log_activity(
        &state.db,
        "message_edited",
        Some(payload.user_id),
        json!({"message_id": updated_message.id, "chat_id": updated_message.chat_id}),
    )
    .await;

    state
        .websocket_broadcaster
        .publish(WsServerEvent::ChatMessage(WsChatMessage {
            chat_id: updated_message.chat_id,
            chat_type: updated_message.chat_type,
            message: WsMessageDto::from(&updated_message),
        }));

    (
        StatusCode::OK,
        Json(ApiEnvelope::success("message edited", json!(updated_message))),
    )
}

/// Soft-deletes a message and broadcasts the update.
pub async fn delete_message_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DeleteMessagePayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }

    let message = state
        .db
        .read(|db| find_message(db, payload.chat_type, payload.message_id).cloned())
        .await;
    let Some(message) = message else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("message not found")),
        );
    };

    if message.sender_id != payload.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "only the sender can delete",
            )),
        );
    }

    if message.deleted {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error("message already deleted")),
        );
    }

    if !has_chat_access(&state, payload.chat_type, message.chat_id, payload.user_id).await {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error("user is not member of this chat")),
        );
    }

    let now = Utc::now();
    let updated_message = state
        .db
        .write(|db| {
            let message = find_message_mut(db, payload.chat_type, payload.message_id)?;
            message.body = "Mesaj silindi".to_string();
            message.deleted = true;
            message.edited_at = Some(now);
            message.mentions.clear();
            Some(message.clone())
        })
        .await;

    let Some(updated_message) = updated_message else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("message not found")),
        );
    };

    let event_row = state
        .events
        .insert_event_row(
            &state.db,
            EventType::MessageDeleted,
            json!({
                "message_id": updated_message.id,
                "chat_id": updated_message.chat_id,
                "chat_type": updated_message.chat_type as u8,
                "deleted_by": payload.user_id,
            }),
        )
        .await;
    state.events.dispatch_after_commit(event_row);

    logging::log_activity(
        &state.db,
        "message_deleted",
        Some(payload.user_id),
        json!({"message_id": updated_message.id, "chat_id": updated_message.chat_id}),
    )
    .await;

    state
        .websocket_broadcaster
        .publish(WsServerEvent::ChatMessage(WsChatMessage {
            chat_id: updated_message.chat_id,
            chat_type: updated_message.chat_type,
            message: WsMessageDto::from(&updated_message),
        }));

    (
        StatusCode::OK,
        Json(ApiEnvelope::success_no_data("message deleted")),
    )
}

/// Starts DM flow: create request for non-friends, direct chat for friends.
pub async fn request_dm_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RequestDmPayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.from_user_id).await
    {
        return response;
    }
    if payload.from_user_id == payload.to_user_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "cannot DM yourself",
            )),
        );
    }

    // Check sender existence + write permission.
    let sender = state
        .db
        .read(|db| db.users.get(&payload.from_user_id).cloned())
        .await;
    let Some(sender) = sender else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("sender not found")),
        );
    };
    if !sender.status.can_write() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "sender cannot start DM requests",
            )),
        );
    }

    // Enforce per-user DM start rate limits.
    if !state
        .rate_limit
        .allow_dm_request(payload.from_user_id, Utc::now())
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "dm start rate limit exceeded",
            )),
        );
    }

    // Branch by friendship state.
    let pair_key = normalize_friend_pair(payload.from_user_id, payload.to_user_id);
    let is_friend = state.db.read(|db| db.friendships.contains(&pair_key)).await;

    if is_friend {
        // Friends get direct chat immediately.
        // Reuse existing DM chat if present.
        let existing_dm = state
            .db
            .read(|db| {
                db.chats.values().find(|chat| {
                    chat.chat_type == ChatType::DirectMessage
                        && {
                            let members: Vec<_> = db
                                .chat_members
                                .iter()
                                .filter(|m| m.chat_id == chat.id)
                                .map(|m| m.user_id)
                                .collect();
                            members.contains(&payload.from_user_id)
                                && members.contains(&payload.to_user_id)
                        }
                }).cloned()
            })
            .await;

        let chat = if let Some(chat) = existing_dm {
            chat
        } else {
            let chat = ChatRoom {
                id: Uuid::new_v4(),
                chat_type: ChatType::DirectMessage,
                created_by: payload.from_user_id,
                created_at: Utc::now(),
                label: None,
            };

            state
                .db
                .write(|db| {
                    db.chats.insert(chat.id, chat.clone());
                    db.chat_members.push(ChatMember {
                        chat_id: chat.id,
                        user_id: payload.from_user_id,
                        joined_at: Utc::now(),
                    });
                    db.chat_members.push(ChatMember {
                        chat_id: chat.id,
                        user_id: payload.to_user_id,
                        joined_at: Utc::now(),
                    });
                })
                .await;

            chat
        };

        let event = state
            .events
            .insert_event_row(
                &state.db,
                EventType::FriendAccepted,
                json!({
                    "from_user_id": payload.from_user_id,
                    "to_user_id": payload.to_user_id,
                    "chat_id": chat.id,
                }),
            )
            .await;
        state.events.dispatch_after_commit(event);

        logging::log_activity(
            &state.db,
            "dm_started_friend",
            Some(payload.from_user_id),
            json!({"chat_id": chat.id, "to": payload.to_user_id}),
        )
        .await;

        return (
            StatusCode::OK,
            Json(ApiEnvelope::success(
                "friends can start direct chat immediately",
                json!({"chat_id": chat.id, "action": "direct_chat"}),
            )),
        );
    }

    // Non-friends create a DM request row.
    // Prevent duplicate/opposite pending requests.
    let has_pending = state
        .db
        .read(|db| {
            db.pending_dm_requests.iter().any(|req| {
                (req.from_user_id == payload.from_user_id && req.to_user_id == payload.to_user_id)
                    || (req.from_user_id == payload.to_user_id && req.to_user_id == payload.from_user_id)
            })
        })
        .await;
    if has_pending {
        return (
            StatusCode::CONFLICT,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "pending dm request already exists between users",
            )),
        );
    }

    let request_row = DmRequestRow {
        id: Uuid::new_v4(),
        from_user_id: payload.from_user_id,
        to_user_id: payload.to_user_id,
        created_at: Utc::now(),
    };

    state
        .db
        .write(|db| {
            db.pending_dm_requests.push(request_row.clone());
        })
        .await;

    let event = state
        .events
        .insert_event_row(
            &state.db,
            EventType::FriendRequested,
            json!({
                "request_id": request_row.id,
                "from_user_id": request_row.from_user_id,
                "to_user_id": request_row.to_user_id,
            }),
        )
        .await;
    state.events.dispatch_after_commit(event);

    logging::log_activity(
        &state.db,
        "dm_requested_non_friend",
        Some(payload.from_user_id),
        json!({"request_id": request_row.id, "to": payload.to_user_id}),
    )
    .await;

    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "dm request created",
            json!({"request_id": request_row.id, "action": "request"}),
        )),
    )
}

/// Creates group chat while enforcing `max 3 groups per user`.
pub async fn create_group_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGroupPayload>,
) -> impl IntoResponse {
    if let Err(response) =
        auth::require_authenticated_user_match_id(&state, &headers, payload.user_id).await
    {
        return response;
    }
    // Validate creator status.
    let creator = state
        .db
        .read(|db| db.users.get(&payload.user_id).cloned())
        .await;
    let Some(creator) = creator else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("creator not found")),
        );
    };
    if !creator.status.can_write() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "creator cannot create groups",
            )),
        );
    }

    // Count existing groups created by this user.
    let group_count = state
        .db
        .read(|db| {
            db.chats
                .values()
                .filter(|chat| {
                    chat.chat_type == ChatType::Group && chat.created_by == payload.user_id
                })
                .count()
        })
        .await;

    if group_count >= 3 {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "group creation limit reached (max 3)",
            )),
        );
    }

    // Need at least creator + 2 invitees = 3 members.
    let mut invitees = payload.member_ids.clone();
    invitees.retain(|id| *id != payload.user_id);
    invitees.sort();
    invitees.dedup();
    if invitees.len() < 2 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "group must include creator + 2 members (3 total)",
            )),
        );
    }

    // Validate invitees exist.
    let missing = state
        .db
        .read(|db| {
            invitees
                .iter()
                .filter(|id| !db.users.contains_key(id))
                .cloned()
                .collect::<Vec<_>>()
        })
        .await;
    if !missing.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("one or more invitees not found")),
        );
    }

    // Create group and memberships.
    let group_id = Uuid::new_v4();
    let group_chat = ChatRoom {
        id: group_id,
        chat_type: ChatType::Group,
        created_by: payload.user_id,
        created_at: Utc::now(),
        label: Some(format!("Grup {}", &group_id.to_string()[..6])),
    };

    state
        .db
        .write(|db| {
            db.chats.insert(group_chat.id, group_chat.clone());
            db.chat_members.push(ChatMember {
                chat_id: group_chat.id,
                user_id: payload.user_id,
                joined_at: Utc::now(),
            });
            for member in &invitees {
                db.chat_members.push(ChatMember {
                    chat_id: group_chat.id,
                    user_id: *member,
                    joined_at: Utc::now(),
                });
            }
        })
        .await;

    logging::log_activity(
        &state.db,
        "group_created",
        Some(payload.user_id),
        json!({"chat_id": group_chat.id}),
    )
    .await;

    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "group created",
            json!({"chat_id": group_chat.id}),
        )),
    )
}

/// Parses `@username` mentions and resolves them to user ids with exact normalized match.
async fn parse_mentions(state: &AppState, body: &str) -> Vec<Uuid> {
    // Extract candidate usernames from tokens beginning with `@`.
    let candidates = mention_candidates(body);

    // Keep insertion order while deduplicating users.
    let mut seen = HashSet::new();
    let mut resolved = Vec::new();

    for candidate in candidates.into_iter().take(5) {
        let normalized = normalize_username(&candidate);

        // Resolve candidate against public username index using case-insensitive exact key.
        let user_id = state
            .db
            .read(|db| db.users_by_public.get(&normalized).copied())
            .await;

        if let Some(user_id) = user_id
            && seen.insert(user_id)
        {
            resolved.push(user_id);
        }
    }

    resolved
}

/// Extracts mention candidates from message body.
fn mention_candidates(body: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    for token in body.split_whitespace() {
        let Some(without_at) = token.strip_prefix('@') else {
            continue;
        };

        // Keep only username-safe chars and trim punctuation around token.
        let normalized = without_at
            .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .chars()
            .take(16)
            .collect::<String>();

        if (3..=16).contains(&normalized.len()) {
            candidates.push(normalized);
        }
    }

    candidates
}

/// Returns immutable reference to a message by id and chat type.
fn find_message<'a>(db: &'a DatabaseState, chat_type: ChatType, message_id: Uuid) -> Option<&'a ChatMessage> {
    match chat_type {
        ChatType::DirectMessage => db.dm_messages.iter().find(|m| m.id == message_id),
        ChatType::Group => db.group_messages.iter().find(|m| m.id == message_id),
        ChatType::Community => db.community_messages.iter().find(|m| m.id == message_id),
    }
}

/// Returns mutable reference to a message by id and chat type.
fn find_message_mut<'a>(
    db: &'a mut DatabaseState,
    chat_type: ChatType,
    message_id: Uuid,
) -> Option<&'a mut ChatMessage> {
    match chat_type {
        ChatType::DirectMessage => db.dm_messages.iter_mut().find(|m| m.id == message_id),
        ChatType::Group => db.group_messages.iter_mut().find(|m| m.id == message_id),
        ChatType::Community => db.community_messages.iter_mut().find(|m| m.id == message_id),
    }
}

/// Checks whether user can access the given chat (membership for non-community chats).
async fn has_chat_access(state: &AppState, chat_type: ChatType, chat_id: Uuid, user_id: Uuid) -> bool {
    if chat_type == ChatType::Community {
        return true;
    }

    state
        .db
        .read(|db| {
            db.chat_members
                .iter()
                .any(|member| member.chat_id == chat_id && member.user_id == user_id)
        })
        .await
}
