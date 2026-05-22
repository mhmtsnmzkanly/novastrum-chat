use axum::{routing::get, Router};

use crate::{app_state::AppState, http::health};

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health::health))
}
