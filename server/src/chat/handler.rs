use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    app_state::AppState,
    auth::service::{authenticate_current_user, CurrentUserError},
    chat::{
        dto::{
            ConversationListQuery, CreateDirectConversationRequest, MessageHistoryQuery,
            SendMessageRequest,
        },
        service::{ChatService, ChatServiceError},
    },
    http::response::{ApiErrorPayload, ApiResponse},
};

pub async fn create_direct_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateDirectConversationRequest>,
) -> Response {
    let requester = match authenticate_current_user(&state, &headers).await {
        Ok(user) => user,
        Err(error) => return auth_error_response(error),
    };

    match ChatService::new(&state)
        .create_or_get_direct_conversation(requester, request)
        .await
    {
        Ok(response) => success(StatusCode::OK, "chat.conversation.direct", response),
        Err(error) => chat_error_response(error),
    }
}

pub async fn list_conversations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConversationListQuery>,
) -> Response {
    let requester = match authenticate_current_user(&state, &headers).await {
        Ok(user) => user,
        Err(error) => return auth_error_response(error),
    };

    match ChatService::new(&state)
        .list_conversations(requester, query)
        .await
    {
        Ok(response) => success(StatusCode::OK, "chat.conversations.list", response),
        Err(error) => chat_error_response(error),
    }
}

pub async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(conversation_id): Path<String>,
    Json(request): Json<SendMessageRequest>,
) -> Response {
    let requester = match authenticate_current_user(&state, &headers).await {
        Ok(user) => user,
        Err(error) => return auth_error_response(error),
    };

    match ChatService::new(&state)
        .send_message(requester, conversation_id, request)
        .await
    {
        Ok(response) => success(StatusCode::OK, "chat.message.created", response),
        Err(error) => chat_error_response(error),
    }
}

pub async fn list_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(conversation_id): Path<String>,
    Query(query): Query<MessageHistoryQuery>,
) -> Response {
    let requester = match authenticate_current_user(&state, &headers).await {
        Ok(user) => user,
        Err(error) => return auth_error_response(error),
    };

    match ChatService::new(&state)
        .list_messages(requester, conversation_id, query)
        .await
    {
        Ok(response) => success(StatusCode::OK, "chat.messages.list", response),
        Err(error) => chat_error_response(error),
    }
}

fn success<T: serde::Serialize>(
    status: StatusCode,
    response_type: &'static str,
    response: T,
) -> Response {
    (
        status,
        Json(ApiResponse::ok(response_type, serde_json::json!(response))),
    )
        .into_response()
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

    error_response(status, response_type, payload)
}

fn chat_error_response(error: ChatServiceError) -> Response {
    let (status, response_type, payload) = match error {
        ChatServiceError::Validation(fields) => (
            StatusCode::BAD_REQUEST,
            "error.validation",
            ApiErrorPayload::with_fields(
                "validation_failed",
                "Validation failed",
                serde_json::json!(fields),
            ),
        ),
        ChatServiceError::CurrentUserCannotWrite => (
            StatusCode::FORBIDDEN,
            "error.forbidden",
            ApiErrorPayload::new("forbidden", "Current user cannot perform this action"),
        ),
        ChatServiceError::CannotMessageSelf => (
            StatusCode::BAD_REQUEST,
            "error.validation",
            ApiErrorPayload::new(
                "cannot_message_self",
                "Cannot create a direct conversation with yourself",
            ),
        ),
        ChatServiceError::UserNotFound => (
            StatusCode::NOT_FOUND,
            "error.not_found",
            ApiErrorPayload::new("user_not_found", "User was not found"),
        ),
        ChatServiceError::ConversationNotFound => (
            StatusCode::NOT_FOUND,
            "error.not_found",
            ApiErrorPayload::new("conversation_not_found", "Conversation was not found"),
        ),
        ChatServiceError::NotConversationMember => (
            StatusCode::FORBIDDEN,
            "error.forbidden",
            ApiErrorPayload::new(
                "not_conversation_member",
                "Current user is not a member of this conversation",
            ),
        ),
        ChatServiceError::MessageCursorNotFound => (
            StatusCode::NOT_FOUND,
            "error.not_found",
            ApiErrorPayload::new("message_cursor_not_found", "Message cursor was not found"),
        ),
        ChatServiceError::TargetUnavailable => (
            StatusCode::FORBIDDEN,
            "error.forbidden",
            ApiErrorPayload::new("forbidden", "Target user cannot be messaged"),
        ),
        ChatServiceError::DmNotAllowed => (
            StatusCode::FORBIDDEN,
            "error.forbidden",
            ApiErrorPayload::new(
                "dm_not_allowed",
                "Direct messages are not allowed by this user",
            ),
        ),
        ChatServiceError::DatabaseUnavailable(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "error.database",
            ApiErrorPayload::new("database_unavailable", message),
        ),
        ChatServiceError::Conflict => (
            StatusCode::CONFLICT,
            "error.conflict",
            ApiErrorPayload::new("conflict", "Direct conversation could not be created"),
        ),
    };

    error_response(status, response_type, payload)
}

fn error_response(
    status: StatusCode,
    response_type: &'static str,
    payload: ApiErrorPayload,
) -> Response {
    (
        status,
        Json(ApiResponse::error(
            response_type,
            serde_json::json!(payload),
        )),
    )
        .into_response()
}
