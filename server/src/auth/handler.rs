use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    app_state::AppState,
    auth::{
        dto::{LoginRequest, RegisterRequest},
        service::{session_cookie_value, LoginError, LoginService, RegisterError, RegisterService},
    },
    http::response::{ApiErrorPayload, ApiResponse},
};

pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Response {
    match RegisterService::new(&state).register(request).await {
        Ok(response) => success(StatusCode::CREATED, "auth.register", response),
        Err(error) => register_error_response(error),
    }
}

pub async fn login(State(state): State<AppState>, Json(request): Json<LoginRequest>) -> Response {
    match LoginService::new(&state).login(request).await {
        Ok(result) => {
            let cookie = session_cookie_value(&result.session_token, &state.config.app_env);
            let mut response = success(StatusCode::OK, "auth.login", result.response);
            match HeaderValue::from_str(&cookie) {
                Ok(value) => {
                    response.headers_mut().insert(SET_COOKIE, value);
                    response
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to build session cookie");
                    internal_error_response()
                }
            }
        }
        Err(error) => login_error_response(error),
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

fn register_error_response(error: RegisterError) -> Response {
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

    error_response(status, response_type, payload)
}

fn login_error_response(error: LoginError) -> Response {
    let (status, response_type, payload) = match error {
        LoginError::Validation(fields) => (
            StatusCode::BAD_REQUEST,
            "error.validation",
            ApiErrorPayload::with_fields(
                "validation_failed",
                "Validation failed",
                serde_json::json!(fields),
            ),
        ),
        LoginError::InvalidCredentials => (
            StatusCode::UNAUTHORIZED,
            "error.auth",
            ApiErrorPayload::new("invalid_credentials", "Invalid credentials"),
        ),
        LoginError::PendingApproval => (
            StatusCode::FORBIDDEN,
            "error.auth",
            ApiErrorPayload::new("pending_approval", "Account is pending approval"),
        ),
        LoginError::Banned => (
            StatusCode::FORBIDDEN,
            "error.auth",
            ApiErrorPayload::new("banned", "Account is banned"),
        ),
        LoginError::DatabaseUnavailable(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "error.database",
            ApiErrorPayload::new("database_unavailable", message),
        ),
        LoginError::Internal => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "error.system",
            ApiErrorPayload::new("internal_error", "Login failed"),
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

fn internal_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "error.system",
        ApiErrorPayload::new("internal_error", "Login failed"),
    )
}
