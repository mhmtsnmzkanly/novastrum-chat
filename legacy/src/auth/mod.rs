use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::Sha256;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    AppState,
    events::EventType,
    logging,
    models::{
        ApiEnvelope, ChatMember, ChatRoom, ChatType, SessionRecord, UserRecord, UserStatus,
        normalize_username, validate_password, validate_username,
    },
    permissions,
};

/// Captcha verify payload used by `index.html` gate.
#[derive(Debug, Deserialize)]
pub struct CaptchaVerifyRequest {
    /// Captcha token submitted by browser.
    pub captcha_token: String,
    /// Optional session id to short-circuit authenticated users into app page.
    pub session_id: Option<String>,
    /// Development checkbox state (simulated captcha imitative).
    #[serde(default)]
    pub human_check: bool,
}

/// Register payload.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// Immutable login username.
    pub login_username: String,
    /// Immutable public username.
    pub public_username: String,
    /// Raw password limited to `a-z0-9`.
    pub password: String,
}

/// Login payload.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Login username.
    pub login_username: String,
    /// Raw password.
    pub password: String,
    /// Optional device label for session management UI.
    pub device_label: Option<String>,
}

/// Password reset payload.
#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    /// Login username for lookup.
    pub login_username: String,
    /// New password following policy.
    pub new_password: String,
    /// PIN code challenge.
    pub pin: String,
}

/// PIN verification payload.
#[derive(Debug, Deserialize)]
pub struct PinVerifyRequest {
    /// Optional session id awaiting PIN verification (cookie fallback supported).
    #[serde(default)]
    pub session_id: Option<String>,
    /// User supplied PIN.
    pub pin: String,
}

/// Single-session logout payload.
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    /// Optional session id to invalidate (cookie fallback supported).
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Multi-session logout payload.
#[derive(Debug, Deserialize)]
pub struct LogoutAllRequest {
    /// User id whose sessions should be removed.
    pub user_id: Uuid,
}

/// Mutable auth-only state (PIN challenges).
#[derive(Debug, Default)]
struct AuthState {
    /// Session id => PIN code.
    pin_challenges: HashMap<String, String>,
    /// User id => fallback reset PIN.
    reset_pins: HashMap<Uuid, String>,
}

/// Authentication service handling hashing/session helper operations.
#[derive(Debug, Clone)]
pub struct AuthService {
    /// Lock-protected auth service state.
    state: Arc<RwLock<AuthState>>,
}

impl AuthService {
    /// Creates auth service with empty challenge stores.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(AuthState::default())),
        }
    }

    /// Hashes plain password with Argon2id.
    pub fn hash_password(&self, plain_password: &str) -> anyhow::Result<String> {
        // Generate a unique salt token derived from random UUID bytes.
        let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
            .map_err(|error| anyhow::anyhow!("failed to create password salt: {error}"))?;

        // Hash password using Argon2id default parameters.
        let hash = Argon2::default()
            .hash_password(plain_password.as_bytes(), &salt)
            .map_err(|error| anyhow::anyhow!("argon2 password hashing failed: {error}"))?
            .to_string();

        Ok(hash)
    }

    /// Verifies plain password against stored Argon2 hash.
    pub fn verify_password(&self, plain_password: &str, stored_hash: &str) -> bool {
        // Parse persisted hash string.
        let parsed = match PasswordHash::new(stored_hash) {
            Ok(value) => value,
            Err(_) => return false,
        };

        // Compare provided password against parsed hash.
        Argon2::default()
            .verify_password(plain_password.as_bytes(), &parsed)
            .is_ok()
    }

    /// Stores temporary PIN challenge for a session.
    pub async fn set_pin_challenge(&self, session_id: &str, pin: &str) {
        let mut guard = self.state.write().await;
        guard
            .pin_challenges
            .insert(session_id.to_string(), pin.to_string());
    }

    /// Validates and clears temporary session PIN challenge.
    pub async fn verify_pin_challenge(&self, session_id: &str, pin: &str) -> bool {
        let mut guard = self.state.write().await;
        match guard.pin_challenges.get(session_id) {
            Some(expected) if expected == pin => {
                guard.pin_challenges.remove(session_id);
                true
            }
            _ => false,
        }
    }

    /// Assigns reset PIN for a user (skeleton for reset flow).
    pub async fn set_reset_pin(&self, user_id: Uuid, pin: &str) {
        let mut guard = self.state.write().await;
        guard.reset_pins.insert(user_id, pin.to_string());
    }

    /// Validates reset PIN for a user.
    pub async fn verify_reset_pin(&self, user_id: Uuid, pin: &str) -> bool {
        let guard = self.state.read().await;
        guard
            .reset_pins
            .get(&user_id)
            .map(|value| value == pin)
            .unwrap_or(false)
    }
}

type HmacSha256 = Hmac<Sha256>;

/// Signs raw session id with HMAC-SHA256 and returns cookie-safe token.
fn sign_session_token(secret: &str, session_id: &str) -> Option<String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(session_id.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Some(format!("{session_id}.{signature}"))
}

/// Verifies signed session token and returns raw session id if valid.
fn verify_session_token(secret: &str, signed_token: &str) -> Option<String> {
    let (session_id, encoded_signature) = signed_token.rsplit_once('.')?;
    if session_id.is_empty() || encoded_signature.is_empty() {
        return None;
    }

    let signature = URL_SAFE_NO_PAD.decode(encoded_signature).ok()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(session_id.as_bytes());
    mac.verify_slice(&signature).ok()?;

    Some(session_id.to_string())
}

/// Extracts a cookie value by key from `Cookie` request header.
fn parse_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        let (key, value) = trimmed.split_once('=')?;
        if key == name {
            return Some(value.to_string());
        }
    }

    None
}

/// Resolves signed session cookie from request headers.
fn signed_session_id_from_headers(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let raw = parse_cookie_value(headers, &state.config.session_cookie_name)?;
    verify_session_token(&state.config.session_cookie_secret, &raw)
}

/// Resolves the authenticated session row from signed session cookie and session rules.
pub async fn authenticated_session_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Option<SessionRecord> {
    // Extract and verify signed session token from cookie header.
    let session_id = signed_session_id_from_headers(state, headers)?;

    // Load session row and ensure it is still valid.
    let session = state
        .db
        .read(|db| db.sessions.get(&session_id).cloned())
        .await?;
    if !session_is_valid(&session, Utc::now(), state.config.session_idle_hours) {
        return None;
    }

    Some(session)
}

/// Resolves the authenticated user from signed session cookie and session rules.
pub async fn authenticated_user_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Option<UserRecord> {
    let session = authenticated_session_from_headers(state, headers).await?;
    // Resolve user row from authenticated session owner id.
    state
        .db
        .read(|db| db.users.get(&session.user_id).cloned())
        .await
}

fn auth_error(status: StatusCode, message: &'static str) -> (StatusCode, Json<ApiEnvelope<Value>>) {
    (status, Json(ApiEnvelope::error(message)))
}

/// Ensures the request carries a valid, active signed session.
pub async fn require_authenticated_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<SessionRecord, (StatusCode, Json<ApiEnvelope<Value>>)> {
    let Some(session) = authenticated_session_from_headers(state, headers).await else {
        return Err(auth_error(
            StatusCode::UNAUTHORIZED,
            "authentication required",
        ));
    };
    Ok(session)
}

/// Asserts that the request carries a valid signed session cookie.
pub async fn require_authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<UserRecord, (StatusCode, Json<ApiEnvelope<Value>>)> {
    let session = require_authenticated_session(state, headers).await?;
    let user = state
        .db
        .read(|db| db.users.get(&session.user_id).cloned())
        .await;
    let Some(user) = user else {
        return Err(auth_error(
            StatusCode::UNAUTHORIZED,
            "authentication required",
        ));
    };

    Ok(user)
}

/// Ensures the authenticated session user matches the supplied identifier.
pub async fn require_authenticated_user_match_id(
    state: &AppState,
    headers: &HeaderMap,
    expected_user_id: Uuid,
) -> Result<UserRecord, (StatusCode, Json<ApiEnvelope<Value>>)> {
    let session = require_authenticated_session(state, headers).await?;
    if session.user_id != expected_user_id {
        return Err(auth_error(StatusCode::FORBIDDEN, "session user mismatch"));
    }

    let user = state
        .db
        .read(|db| db.users.get(&session.user_id).cloned())
        .await;
    let Some(user) = user else {
        return Err(auth_error(
            StatusCode::UNAUTHORIZED,
            "session user mismatch",
        ));
    };

    Ok(user)
}

/// Builds `Set-Cookie` header for active session.
fn build_session_set_cookie(state: &AppState, signed_token: &str) -> String {
    let max_age_seconds = state
        .config
        .session_max_days
        .saturating_mul(24)
        .saturating_mul(3600);

    let mut cookie = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        state.config.session_cookie_name, signed_token, max_age_seconds
    );
    if state.config.session_cookie_secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Builds `Set-Cookie` header that expires session cookie immediately.
fn build_session_clear_cookie(state: &AppState) -> String {
    let mut cookie = format!(
        "{}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
        state.config.session_cookie_name
    );
    if state.config.session_cookie_secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Checks whether session is alive using hard-expiry and inactivity rules.
fn session_is_valid(
    session: &SessionRecord,
    now: chrono::DateTime<Utc>,
    max_idle_hours: i64,
) -> bool {
    // Reject session that exceeded hard expiration timestamp.
    if now > session.expires_at {
        return false;
    }

    // Reject session that exceeded inactivity timeout.
    now <= session.last_activity_at + Duration::hours(max_idle_hours)
}

/// Handles captcha flow from `index.html`.
pub async fn verify_captcha_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CaptchaVerifyRequest>,
) -> impl IntoResponse {
    // Reject invalid captcha token before any session logic.
    if payload.captcha_token != state.config.captcha_dev_secret {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "captcha verification failed",
            )),
        );
    }

    if !payload.human_check {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "human checkbox must be checked",
            )),
        );
    }

    // Optionally check provided session for direct app redirect.
    let resolved_session = payload
        .session_id
        .or_else(|| signed_session_id_from_headers(&state, &headers));

    let valid_session = match resolved_session {
        Some(ref session_id) => state
            .db
            .read(|db| db.sessions.get(session_id).cloned())
            .await
            .map(|session| session_is_valid(&session, Utc::now(), state.config.session_idle_hours))
            .unwrap_or(false),
        None => false,
    };

    // Return next page according to session validity result.
    let next_page = if valid_session {
        "app.html"
    } else {
        "auth.html"
    };
    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "captcha verified",
            json!({"next_page": next_page}),
        )),
    )
}

/// Registers a new user with `Pending` status.
pub async fn register_handler(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> impl IntoResponse {
    let login_username = normalize_username(&payload.login_username);
    let public_username = normalize_username(&payload.public_username);

    // Enforce immutable username constraints.
    if !validate_username(&login_username) || !validate_username(&public_username) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "username must be 3-16 chars and use [a-z0-9_]",
            )),
        );
    }

    // Enforce strict password character policy.
    if !validate_password(&payload.password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "password policy allows only [a-z0-9]",
            )),
        );
    }

    // Reject duplicate login/public usernames.
    let duplicate = state
        .db
        .read(|db| {
            db.users_by_login.contains_key(&login_username)
                || db.users_by_public.contains_key(&public_username)
        })
        .await;
    if duplicate {
        return (
            StatusCode::CONFLICT,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "username already exists",
            )),
        );
    }

    // Hash password with Argon2id before persistence.
    let password_hash = match state.auth.hash_password(&payload.password) {
        Ok(hash) => hash,
        Err(err) => {
            tracing::error!(error = %err, "password hashing failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "failed to create account",
                )),
            );
        }
    };

    // Insert `Pending` user according to registration workflow.
    let user = UserRecord {
        id: Uuid::new_v4(),
        login_username: login_username.clone(),
        public_username,
        password_hash,
        status: UserStatus::Pending,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    state
        .db
        .write(|db| {
            db.users_by_login.insert(login_username, user.id);
            db.users_by_public
                .insert(user.public_username.clone(), user.id);
            db.users.insert(user.id, user.clone());
        })
        .await;

    // Seed a deterministic PIN only for the skeleton reset flow.
    state.auth.set_reset_pin(user.id, "000000").await;

    // Log registration as pending approval.
    logging::log_activity(
        &state.db,
        "user_registered_pending",
        Some(user.id),
        json!({"status": "Pending"}),
    )
    .await;

    (
        StatusCode::OK,
        Json(ApiEnvelope::success(
            "registration accepted; waiting for admin approval",
            json!({"user_id": user.id, "status": "Pending"}),
        )),
    )
}

/// Performs login with rate limit, status checks, and session creation.
pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let now = Utc::now();
    let normalized_login = normalize_username(&payload.login_username);

    // Reject attempts while login lockout is active.
    if let Err(remaining) = state
        .rate_limit
        .can_attempt_login(&normalized_login, now)
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiEnvelope::<serde_json::Value>::error(format!(
                "login locked; retry in {remaining} seconds"
            ))),
        )
            .into_response();
    }

    // Load account by login index.
    let user = state
        .db
        .read(|db| {
            db.users_by_login
                .get(&normalized_login)
                .and_then(|id| db.users.get(id))
                .cloned()
        })
        .await;

    let Some(user) = user else {
        state
            .rate_limit
            .record_login_failure(&normalized_login, now)
            .await;
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "invalid credentials",
            )),
        )
            .into_response();
    };

    // Validate Argon2id password hash.
    if !state
        .auth
        .verify_password(&payload.password, &user.password_hash)
    {
        let lock_seconds = state
            .rate_limit
            .record_login_failure(&normalized_login, now)
            .await;

        let message = if lock_seconds > 0 {
            format!("invalid credentials; locked for {lock_seconds} seconds")
        } else {
            "invalid credentials".to_string()
        };

        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiEnvelope::<serde_json::Value>::error(message)),
        )
            .into_response();
    }

    // Account status controls login capabilities.
    if !user.status.can_login() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "account status does not allow login",
            )),
        )
            .into_response();
    }

    // Clear failed-login counters after successful authentication.
    state
        .rate_limit
        .clear_login_failures(&normalized_login)
        .await;

    // Create a DB-backed session with hard expiry + idle timeout semantics.
    let session = SessionRecord {
        session_id: Uuid::new_v4().to_string(),
        user_id: user.id,
        created_at: now,
        last_activity_at: now,
        expires_at: now + Duration::days(state.config.session_max_days),
        device_label: payload.device_label.clone(),
    };

    // Persist session row.
    state
        .db
        .write(|db| {
            db.sessions
                .insert(session.session_id.clone(), session.clone());
        })
        .await;

    // Save a development PIN challenge (auth.html exposes PIN entry form).
    state
        .auth
        .set_pin_challenge(&session.session_id, "000000")
        .await;

    // Record login in activity logs.
    logging::log_activity(
        &state.db,
        "user_logged_in",
        Some(user.id),
        json!({"session_id": session.session_id, "status": format!("{:?}", user.status)}),
    )
    .await;

    // Sign session id and set secure HttpOnly cookie.
    let Some(signed_session) =
        sign_session_token(&state.config.session_cookie_secret, &session.session_id)
    else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "failed to sign session cookie",
            )),
        )
            .into_response();
    };
    let cookie_header = build_session_set_cookie(&state, &signed_session);

    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie_header)],
        Json(ApiEnvelope::success(
            "login successful",
            json!({
                "session_id": session.session_id,
                "redirect": "app.html",
                "status": format!("{:?}", user.status),
                "can_write": user.status.can_write()
            }),
        )),
    )
        .into_response()
}

/// Validates PIN challenge for existing session.
pub async fn pin_verify_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PinVerifyRequest>,
) -> impl IntoResponse {
    let session_id = payload
        .session_id
        .or_else(|| signed_session_id_from_headers(&state, &headers));
    let Some(session_id) = session_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "session not provided",
            )),
        );
    };

    // Confirm session row exists before PIN check.
    let exists = state
        .db
        .read(|db| db.sessions.contains_key(&session_id))
        .await;
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("session not found")),
        );
    }

    // Validate challenge PIN and consume it on success.
    if !state
        .auth
        .verify_pin_challenge(&session_id, &payload.pin)
        .await
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiEnvelope::<serde_json::Value>::error("invalid pin")),
        );
    }

    (
        StatusCode::OK,
        Json(ApiEnvelope::success_no_data("pin verified")),
    )
}

/// Performs password reset using PIN verification.
pub async fn reset_password_handler(
    State(state): State<AppState>,
    Json(payload): Json<ResetPasswordRequest>,
) -> impl IntoResponse {
    let normalized_login = normalize_username(&payload.login_username);

    // Enforce strict password policy before expensive operations.
    if !validate_password(&payload.new_password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "password policy allows only [a-z0-9]",
            )),
        );
    }

    // Load user by login for reset process.
    let user = state
        .db
        .read(|db| {
            db.users_by_login
                .get(&normalized_login)
                .and_then(|id| db.users.get(id))
                .cloned()
        })
        .await;
    let Some(user) = user else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("user not found")),
        );
    };

    // Validate reset PIN for user.
    if !state.auth.verify_reset_pin(user.id, &payload.pin).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiEnvelope::<serde_json::Value>::error("invalid reset pin")),
        );
    }

    // Re-hash and persist new password.
    let new_hash = match state.auth.hash_password(&payload.new_password) {
        Ok(hash) => hash,
        Err(err) => {
            tracing::error!(error = %err, "password hashing failed on reset");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiEnvelope::<serde_json::Value>::error(
                    "failed to reset password",
                )),
            );
        }
    };

    // Update user password hash atomically.
    state
        .db
        .write(|db| {
            if let Some(stored) = db.users.get_mut(&user.id) {
                stored.password_hash = new_hash;
                stored.updated_at = Utc::now();
            }
        })
        .await;

    // Log password reset for audit.
    logging::log_activity(
        &state.db,
        "password_reset",
        Some(user.id),
        json!({"login": user.login_username}),
    )
    .await;

    (
        StatusCode::OK,
        Json(ApiEnvelope::success_no_data("password reset successful")),
    )
}

/// Logs out the current device/session.
pub async fn logout_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LogoutRequest>,
) -> impl IntoResponse {
    let session_id = payload
        .session_id
        .or_else(|| signed_session_id_from_headers(&state, &headers));
    let Some(session_id) = session_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiEnvelope::<serde_json::Value>::error(
                "session not provided",
            )),
        )
            .into_response();
    };

    // Remove session row and return whether operation affected a row.
    let removed = state.db.write(|db| db.sessions.remove(&session_id)).await;

    let Some(session) = removed else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiEnvelope::<serde_json::Value>::error("session not found")),
        )
            .into_response();
    };

    // Log single-session logout event.
    logging::log_activity(
        &state.db,
        "user_logged_out_current",
        Some(session.user_id),
        json!({"session_id": session_id}),
    )
    .await;

    let clear_cookie = build_session_clear_cookie(&state);

    (
        StatusCode::OK,
        [(header::SET_COOKIE, clear_cookie)],
        Json(ApiEnvelope::<serde_json::Value>::success_no_data(
            "logged out current device",
        )),
    )
        .into_response()
}

/// Logs out all sessions for one user id.
pub async fn logout_all_handler(
    State(state): State<AppState>,
    Json(payload): Json<LogoutAllRequest>,
) -> impl IntoResponse {
    // Remove all sessions that belong to requested user.
    let removed_count = state
        .db
        .write(|db| {
            let before = db.sessions.len();
            db.sessions
                .retain(|_, session| session.user_id != payload.user_id);
            before.saturating_sub(db.sessions.len())
        })
        .await;

    // Log global logout operation for audit.
    logging::log_activity(
        &state.db,
        "user_logged_out_all",
        Some(payload.user_id),
        json!({"removed_sessions": removed_count}),
    )
    .await;

    let clear_cookie = build_session_clear_cookie(&state);

    (
        StatusCode::OK,
        [(header::SET_COOKIE, clear_cookie)],
        Json(ApiEnvelope::success(
            "logged out all devices",
            json!({"removed_sessions": removed_count}),
        )),
    )
        .into_response()
}

/// Inserts bootstrap users/chats so local environment can be tested immediately.
pub async fn bootstrap_seed_data(state: &AppState) -> anyhow::Result<()> {
    // Skip seeding when users already exist.
    let already_seeded = state.db.read(|db| !db.users.is_empty()).await;
    if already_seeded {
        return Ok(());
    }

    // Build initial admin account.
    let admin_id = Uuid::new_v4();
    let admin_hash = state
        .auth
        .hash_password("admin123")
        .context("failed to hash bootstrap admin password")?;

    let admin_user = UserRecord {
        id: admin_id,
        login_username: "admin".to_string(),
        public_username: "admin".to_string(),
        password_hash: admin_hash,
        status: UserStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Build suspended example account for status behavior demos.
    let suspended_id = Uuid::new_v4();
    let suspended_hash = state
        .auth
        .hash_password("suspended1")
        .context("failed to hash bootstrap suspended password")?;

    let suspended_user = UserRecord {
        id: suspended_id,
        login_username: "suspended".to_string(),
        public_username: "suspended".to_string(),
        password_hash: suspended_hash,
        status: UserStatus::Suspended,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Build default community chats.
    let community_chats = [
        ("Genel", Uuid::new_v4()),
        ("Programlama", Uuid::new_v4()),
        ("Yapay Zeka", Uuid::new_v4()),
    ]
    .into_iter()
    .map(|(label, id)| ChatRoom {
        id,
        chat_type: ChatType::Community,
        created_by: admin_id,
        created_at: Utc::now(),
        label: Some(label.to_string()),
    })
    .collect::<Vec<_>>();

    // Persist seed rows in a single write operation.
    state
        .db
        .write(|db| {
            db.users_by_login.insert("admin".to_string(), admin_id);
            db.users_by_public.insert("admin".to_string(), admin_id);
            db.users.insert(admin_id, admin_user.clone());

            db.users_by_login
                .insert("suspended".to_string(), suspended_id);
            db.users_by_public
                .insert("suspended".to_string(), suspended_id);
            db.users.insert(suspended_id, suspended_user.clone());

            db.chats.insert(community_chats[0].id, community_chats[0].clone());
            db.chat_members.push(ChatMember {
                chat_id: community_chats[0].id,
                user_id: admin_id,
                joined_at: Utc::now(),
            });
            db.chat_members.push(ChatMember {
                chat_id: community_chats[0].id,
                user_id: suspended_id,
                joined_at: Utc::now(),
            });
        })
        .await;

    // Seed reset PINs for bootstrap accounts.
    state.auth.set_reset_pin(admin_id, "000000").await;
    state.auth.set_reset_pin(suspended_id, "000000").await;

    // Seed default permissions and upload levels for admin.
    state
        .permissions
        .seed_default_permissions(&state.db, admin_id)
        .await;
    state
        .permissions
        .grant(&state.db, admin_id, permissions::UPLOAD_FILE_LEVEL_2)
        .await;
    state
        .permissions
        .grant(&state.db, admin_id, permissions::UPLOAD_FILE_LEVEL_3)
        .await;

    // Write bootstrap activity logs.
    logging::log_activity(
        &state.db,
        "bootstrap_seed",
        Some(admin_id),
        json!({"community_chat_id": community_chats[0].id}),
    )
    .await;

    // Emit synthetic friend-accepted event to prove event pipeline at startup.
    let event = state
        .events
        .insert_event_row(
            &state.db,
            EventType::FriendAccepted,
            json!({"seed": true, "user_id": admin_id}),
        )
        .await;
    state.events.dispatch_after_commit(event);

    Ok(())
}
