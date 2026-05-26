use axum::{routing::post, Router};

use crate::{app_state::AppState, chat::handler};

pub fn router() -> Router<AppState> {
    Router::new().route("/direct", post(handler::create_direct_conversation))
}
