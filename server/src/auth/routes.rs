use axum::{routing::post, Router};

use crate::{app_state::AppState, auth::handler};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(handler::register))
        .route("/login", post(handler::login))
        .route("/logout", post(handler::logout))
}
