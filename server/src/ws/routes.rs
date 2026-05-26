use axum::{routing::get, Router};

use crate::{app_state::AppState, ws::handler};

pub fn router() -> Router<AppState> {
    Router::new().route("/ws", get(handler::websocket))
}
