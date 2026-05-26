use axum::{
    routing::{get, post},
    Router,
};

use crate::{app_state::AppState, chat::handler};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(handler::list_conversations))
        .route("/direct", post(handler::create_direct_conversation))
        .route(
            "/:conversation_id/messages",
            get(handler::list_messages).post(handler::send_message),
        )
}
