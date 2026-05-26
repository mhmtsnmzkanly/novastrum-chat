use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    app_state::AppState,
    auth::{
        current_user::AuthenticatedUser,
        service::{authenticate_current_user, CurrentUserError},
    },
    http::response::{ApiErrorPayload, ApiResponse},
    ws::{
        hub::WsHub,
        protocol::{
            connected_packet, unknown_packet_type_packet, ClientPacket, OutboundPacket,
            ServerPacket,
        },
    },
};

pub async fn websocket(
    State(state): State<AppState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let user = match authenticate_current_user(&state, &headers).await {
        Ok(user) => user,
        Err(error) => return auth_error_response(error),
    };

    let hub = state.ws_hub.clone();

    upgrade.on_upgrade(move |socket| handle_socket(socket, hub, user))
}

async fn handle_socket(mut socket: WebSocket, hub: WsHub, user: AuthenticatedUser) {
    let user_id = user.id;
    let registered = hub.register(user_id).await;
    let connection_id = registered.connection_id;
    let mut outbound = registered.receiver;

    if send_packet(&mut socket, &connected_packet(user))
        .await
        .is_err()
    {
        hub.unregister(user_id, connection_id).await;
        return;
    }

    loop {
        tokio::select! {
            inbound = socket.recv() => {
                let Some(result) = inbound else {
                    break;
                };
                let message = match result {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::debug!(%error, "websocket receive failed");
                        break;
                    }
                };

                if !handle_inbound_message(&mut socket, message).await {
                    break;
                }
            }
            outbound_packet = outbound.recv() => {
                match outbound_packet {
                    Some(packet) => {
                        if send_outbound_packet(&mut socket, packet).await.is_err() {
                            break;
                        }
                    }
                    None => {
                        break;
                    }
                }
            }
        }
    }

    hub.unregister(user_id, connection_id).await;
}

async fn handle_inbound_message(socket: &mut WebSocket, message: Message) -> bool {
    match message {
        Message::Text(text) => match serde_json::from_str::<ClientPacket>(&text) {
            Ok(packet) => {
                tracing::debug!(packet_type = %packet.packet_type, "received unsupported websocket packet");
                send_packet(socket, &unknown_packet_type_packet())
                    .await
                    .is_ok()
            }
            Err(error) => {
                tracing::debug!(%error, "closing websocket after invalid json packet");
                let _ = socket.send(Message::Close(None)).await;
                false
            }
        },
        Message::Binary(_) => {
            tracing::debug!("closing websocket after unsupported binary packet");
            let _ = socket.send(Message::Close(None)).await;
            false
        }
        Message::Close(_) => false,
        Message::Ping(payload) => socket.send(Message::Pong(payload)).await.is_ok(),
        Message::Pong(_) => true,
    }
}

async fn send_packet<T>(socket: &mut WebSocket, packet: &ServerPacket<T>) -> Result<(), axum::Error>
where
    T: serde::Serialize,
{
    let text = serde_json::to_string(packet).map_err(axum::Error::new)?;
    socket.send(Message::Text(text)).await
}

async fn send_outbound_packet(
    socket: &mut WebSocket,
    packet: OutboundPacket,
) -> Result<(), axum::Error> {
    let text = serde_json::to_string(&packet).map_err(axum::Error::new)?;
    socket.send(Message::Text(text)).await
}

fn auth_error_response(error: CurrentUserError) -> Response {
    let (status, response_type, payload) = match error {
        CurrentUserError::AuthRequired => (
            StatusCode::UNAUTHORIZED,
            "error.auth",
            ApiErrorPayload::new("auth_required", "Authentication is required"),
        ),
        CurrentUserError::PendingApproval => (
            StatusCode::FORBIDDEN,
            "error.auth",
            ApiErrorPayload::new("pending_approval", "Account is pending approval"),
        ),
        CurrentUserError::Banned => (
            StatusCode::FORBIDDEN,
            "error.auth",
            ApiErrorPayload::new("banned", "Account is banned"),
        ),
        CurrentUserError::DatabaseUnavailable(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "error.database",
            ApiErrorPayload::new("database_unavailable", message),
        ),
        CurrentUserError::Internal => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "error.system",
            ApiErrorPayload::new("internal_error", "Authentication failed"),
        ),
    };

    (
        status,
        Json(ApiResponse::error(
            response_type,
            serde_json::json!(payload),
        )),
    )
        .into_response()
}
