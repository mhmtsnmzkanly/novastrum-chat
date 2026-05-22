use axum::{extract::State, http::StatusCode, Json};

use crate::{
    app_state::AppState,
    auth::{
        dto::{RegisterRequest, RegisterResponse},
        service::{RegisterError, RegisterService},
    },
    http::response::{ApiErrorPayload, ApiResponse},
};

pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    match RegisterService::new(&state).register(request).await {
        Ok(response) => success(StatusCode::CREATED, "auth.register", response),
        Err(error) => error_response(error),
    }
}

fn success(
    status: StatusCode,
    response_type: &'static str,
    response: RegisterResponse,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    (
        status,
        Json(ApiResponse::ok(response_type, serde_json::json!(response))),
    )
}

fn error_response(error: RegisterError) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    let (status, response_type, payload) = match error {
        RegisterError::Validation(fields) => (
            StatusCode::BAD_REQUEST,
            "error.validation",
            ApiErrorPayload::with_fields(
                "validation_failed",
                "Validation failed",
                serde_json::json!(fields),
            ),
        ),
        RegisterError::InviteRequired => (
            StatusCode::FORBIDDEN,
            "error.forbidden",
            ApiErrorPayload::new(
                "invite_required",
                "Invite-only registration is not available yet",
            ),
        ),
        RegisterError::DatabaseUnavailable(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "error.database",
            ApiErrorPayload::new("database_unavailable", message),
        ),
        RegisterError::UserNameTaken => (
            StatusCode::CONFLICT,
            "error.conflict",
            ApiErrorPayload::new("user_name_taken", "User name is already taken"),
        ),
        RegisterError::Internal => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "error.system",
            ApiErrorPayload::new("internal_error", "Registration failed"),
        ),
    };

    (
        status,
        Json(ApiResponse::error(
            response_type,
            serde_json::json!(payload),
        )),
    )
}
