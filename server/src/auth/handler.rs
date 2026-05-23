use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    app_state::AppState,
    auth::{
        dto::{LoginRequest, RegisterRequest},
        service::{
            clear_session_cookie_value, session_cookie_value, CurrentUserError, CurrentUserService,
            LoginError, LoginService, LogoutError, LogoutService, RegisterError, RegisterService,
            SESSION_COOKIE_NAME,
        },
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
            let response = success(StatusCode::OK, "auth.login", result.response);
            response_with_cookie(response, &cookie, "failed to build session cookie")
        }
        Err(error) => login_error_response(error),
    }
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let session_token = session_token_from_headers(&headers);

    match CurrentUserService::new(&state)
        .current_user(session_token.as_deref())
        .await
    {
        Ok(response) => success(StatusCode::OK, "auth.me", response),
        Err(error) => current_user_error_response(error),
    }
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let session_token = session_token_from_headers(&headers);
    let clear_cookie = clear_session_cookie_value(&state.config.app_env);

    let response = match LogoutService::new(&state)
        .logout(session_token.as_deref())
        .await
    {
        Ok(response) => success(StatusCode::OK, "auth.logout", response),
        Err(error) => logout_error_response(error),
    };

    response_with_cookie(
        response,
        &clear_cookie,
        "failed to build clearing session cookie",
    )
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

fn current_user_error_response(error: CurrentUserError) -> Response {
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
            ApiErrorPayload::new("internal_error", "Current user lookup failed"),
        ),
    };

    error_response(status, response_type, payload)
}

fn logout_error_response(error: LogoutError) -> Response {
    let (status, response_type, payload) = match error {
        LogoutError::DatabaseUnavailable(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "error.database",
            ApiErrorPayload::new("database_unavailable", message),
        ),
        LogoutError::Internal => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "error.system",
            ApiErrorPayload::new("internal_error", "Logout failed"),
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

fn response_with_cookie(
    mut response: Response,
    cookie: &str,
    log_message: &'static str,
) -> Response {
    match HeaderValue::from_str(cookie) {
        Ok(value) => {
            response.headers_mut().insert(SET_COOKIE, value);
            response
        }
        Err(error) => {
            tracing::warn!(%error, log_message);
            internal_error_response("Request failed")
        }
    }
}

fn internal_error_response(message: &'static str) -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "error.system",
        ApiErrorPayload::new("internal_error", message),
    )
}

fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;

    cookie_header.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == SESSION_COOKIE_NAME && !value.is_empty()).then(|| value.to_string())
    })
}
