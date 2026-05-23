use axum::{routing::get, Router};

use crate::{app_state::AppState, auth, http::health};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/health/db", get(health::database_health))
        .route("/api/me", get(auth::handler::me))
        .nest("/api/auth", auth::routes::router())
}
