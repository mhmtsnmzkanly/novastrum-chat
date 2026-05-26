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
    auth::service::{authenticate_current_user, CurrentUserError},
    http::response::{ApiErrorPayload, ApiResponse},
    ws::protocol::{connected_packet, unknown_packet_type_packet, ClientPacket, ServerPacket},
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

    upgrade.on_upgrade(move |socket| handle_socket(socket, user))
}

async fn handle_socket(mut socket: WebSocket, user: crate::auth::current_user::AuthenticatedUser) {
    if send_packet(&mut socket, &connected_packet(user))
        .await
        .is_err()
    {
        return;
    }

    while let Some(result) = socket.recv().await {
        let message = match result {
            Ok(message) => message,
            Err(error) => {
                tracing::debug!(%error, "websocket receive failed");
                break;
            }
        };

        match message {
            Message::Text(text) => match serde_json::from_str::<ClientPacket>(&text) {
                Ok(packet) => {
                    tracing::debug!(packet_type = %packet.packet_type, "received unsupported websocket packet");
                    if send_packet(&mut socket, &unknown_packet_type_packet())
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    tracing::debug!(%error, "closing websocket after invalid json packet");
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }
            },
            Message::Binary(_) => {
                tracing::debug!("closing websocket after unsupported binary packet");
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            Message::Close(_) => break,
            Message::Ping(payload) => {
                if socket.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Message::Pong(_) => {}
        }
    }
}

async fn send_packet<T>(socket: &mut WebSocket, packet: &ServerPacket<T>) -> Result<(), axum::Error>
where
    T: serde::Serialize,
{
    let text = serde_json::to_string(packet).map_err(axum::Error::new)?;
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
