mod admin;
mod auth;
mod backup;
mod chats;
mod community;
mod config;
mod db;
mod discussion;
mod events;
mod files;
mod health;
mod logging;
mod models;
mod notifications;
mod permissions;
mod presence;
mod rate_limit;
mod websocket;

use std::{path::PathBuf, time::Instant};

use anyhow::Context;
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use config::AppConfig;
use tokio::fs;
use tower_http::{services::ServeDir, trace::TraceLayer};

/// Global application state shared by all request handlers.
#[derive(Clone)]
pub struct AppState {
    /// Immutable runtime configuration loaded at startup.
    pub config: AppConfig,
    /// Shared PostgreSQL-backed state store.
    pub db: db::Database,
    /// Authentication service (captcha, register, login, session flows).
    pub auth: auth::AuthService,
    /// Permission checker (role-less permission table design).
    pub permissions: permissions::PermissionService,
    /// Rate limiter service (login, message, vote, DM request).
    pub rate_limit: rate_limit::RateLimitService,
    /// Presence tracker for active WebSocket sessions.
    pub presence: presence::PresenceService,
    /// Event audit logger + async dispatcher.
    pub events: events::EventService,
    /// Notification manager (mute, mention override, unread counter).
    pub notifications: notifications::NotificationService,
    /// Backup scheduler for pg_dump exports.
    pub backup: backup::BackupService,
    /// Process start timestamp used by `/health` responses.
    pub started_at: Instant,
    /// Broadcaster used to send server-originated websocket events.
    pub websocket_broadcaster: websocket::WebSocketBroadcaster,
}

impl AppState {
    /// Constructs every shared service using a single config snapshot.
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        // Initialize PostgreSQL-backed state store before creating dependent services.
        let db = db::Database::new(&config.database_url).await?;

        let backup_config = config.clone();
        Ok(Self {
            config,
            db,
            auth: auth::AuthService::new(),
            permissions: permissions::PermissionService::new(),
            rate_limit: rate_limit::RateLimitService::new(),
            presence: presence::PresenceService::new(),
            events: events::EventService::new(),
            notifications: notifications::NotificationService::new(),
            backup: backup::BackupService::new(backup_config),
            started_at: Instant::now(),
            websocket_broadcaster: websocket::WebSocketBroadcaster::new(),
        })
    }
}

/// Builds the HTTP router and binds each spec-driven module endpoint.
fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/app.html", get(app_html_handler))
        .route("/App.html", get(app_html_handler))
        .route("/admin.html", get(admin_html_handler))
        .route("/health", get(health::health_handler))
        .route("/api/bootstrap/state", get(health::bootstrap_state_handler))
        .route("/ws", get(websocket::ws_handler))
        .route("/api/captcha/verify", post(auth::verify_captcha_handler))
        .route("/api/auth/register", post(auth::register_handler))
        .route("/api/auth/login", post(auth::login_handler))
        .route("/api/auth/reset", post(auth::reset_password_handler))
        .route("/api/auth/pin", post(auth::pin_verify_handler))
        .route("/api/auth/logout", post(auth::logout_handler))
        .route("/api/auth/logout-all", post(auth::logout_all_handler))
        .route("/api/chats/message", post(chats::send_message_handler))
        .route("/api/chats/message/edit", post(chats::edit_message_handler))
        .route("/api/chats/message/delete", post(chats::delete_message_handler))
        .route("/api/chats/message/history", get(chats::list_messages_handler))
        .route("/api/chats/dm-request", post(chats::request_dm_handler))
        .route("/api/chats/dm-request/list", get(chats::list_dm_requests_handler))
        .route("/api/chats/dm-request/accept", post(chats::accept_dm_request_handler))
        .route("/api/chats/dm-request/reject", post(chats::reject_dm_request_handler))
        .route("/api/chats/group", post(chats::create_group_handler))
        .route("/api/chats/group/rename", post(chats::rename_group_handler))
        .route("/api/chats/group/member/add", post(chats::add_group_member_handler))
        .route("/api/chats/group/member/remove", post(chats::remove_group_member_handler))
        .route("/api/chats/group/delete", post(chats::delete_group_handler))
        .route(
            "/api/community/mute",
            post(community::mute_community_handler),
        )
        .route(
            "/api/discussion/post",
            post(discussion::create_post_handler),
        )
        .route("/api/discussion/vote", post(discussion::vote_handler))
        .route(
            "/api/discussion/edit-request",
            post(discussion::request_edit_handler),
        )
        .route("/api/discussion/list", get(discussion::list_posts_handler))
        .route("/api/files/prepare", post(files::prepare_upload_handler))
        .route("/api/files/upload", post(files::upload_file_handler))
        .route(
            "/api/notifications/list",
            get(notifications::list_notifications_handler),
        )
        .route(
            "/api/notifications/unread",
            get(notifications::unread_count_handler),
        )
        .route(
            "/api/presence/online",
            get(presence::presence_status_handler),
        )
        .route("/api/backup/plan", get(backup::backup_plan_handler))
        .route("/api/admin/overview", get(admin::overview_handler))
        .route("/api/admin/users", get(admin::list_users_handler))
        .route(
            "/api/admin/users/approve",
            post(admin::approve_user_handler),
        )
        .route(
            "/api/admin/users/status",
            post(admin::set_user_status_handler),
        )
        .route("/api/admin/users/reject", post(admin::reject_user_handler))
        .route(
            "/api/admin/users/permission",
            post(admin::toggle_permission_handler),
        )
        .route(
            "/api/admin/rate-config",
            get(admin::get_rate_config_handler),
        )
        .route(
            "/api/admin/rate-config",
            post(admin::update_rate_config_handler),
        )
        .route("/api/admin/audit", get(admin::audit_handler))
        .route(
            "/api/admin/discussion/edits",
            get(admin::list_edit_requests_handler),
        )
        .route(
            "/api/admin/discussion/edits/decide",
            post(admin::decide_edit_request_handler),
        )
        // The web directory provides index/auth/app pages and static assets.
        .fallback_service(ServeDir::new("web").append_index_html_on_directories(true))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn serve_html_file(filename: &str) -> Response {
    let page_path = PathBuf::from("web").join(filename);
    match fs::read_to_string(&page_path).await {
        Ok(body) => Html(body).into_response(),
        Err(error) => {
            tracing::error!(?error, path = %page_path.display(), "failed to load static html page");
            (StatusCode::INTERNAL_SERVER_ERROR, "static page unavailable").into_response()
        }
    }
}

async fn app_html_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if auth::authenticated_session_from_headers(&state, &headers)
        .await
        .is_none()
    {
        return Redirect::temporary("/index.html").into_response();
    }

    serve_html_file("app.html").await
}

async fn admin_html_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if admin::require_admin_user(&state, &headers).await.is_err() {
        return Redirect::temporary("/auth.html").into_response();
    }

    serve_html_file("admin.html").await
}

/// Waits for SIGINT/SIGTERM and triggers graceful server shutdown.
async fn shutdown_signal() {
    // Wait for Ctrl+C from terminal.
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %error, "failed to listen for ctrl_c");
        }
    };

    // Wait for SIGTERM when running on Unix-like systems.
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => {
                tracing::error!(error = %error, "failed to listen for sigterm");
            }
        }
    };

    // On non-Unix systems keep terminate branch pending forever.
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // Race signal listeners and continue when first shutdown signal arrives.
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    tracing::info!("shutdown signal received, starting graceful shutdown");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load `.env` file when present so local development does not require manual `export` commands.
    let _ = dotenvy::dotenv();

    // Structured logging is initialized first so boot failures are visible.
    logging::init_logging();

    // Load runtime configuration from environment with safe defaults.
    let config = AppConfig::from_env().context("failed to load configuration")?;

    // Create shared application state used by all modules.
    let state = AppState::new(config.clone())
        .await
        .context("failed to build app state")?;

    // Bootstrap default permission grants and demo seed accounts for first start.
    auth::bootstrap_seed_data(&state).await?;

    // Keep DB handle for explicit pool close after server loop exits.
    let db_for_shutdown = state.db.clone();

    // Build the final router with API + WebSocket + static web pages.
    let app = build_router(state);

    // Bind TCP listener to configured address.
    let listener = tokio::net::TcpListener::bind(&config.http_bind)
        .await
        .with_context(|| format!("failed to bind address {}", config.http_bind))?;

    tracing::info!(address = %config.http_bind, "novastrum-chat server started");

    // Run the server and stop accepting new work after shutdown signal.
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum server stopped unexpectedly")?;

    // Close PostgreSQL connections gracefully before process exit.
    db_for_shutdown.pool().close().await;

    Ok(())
}
