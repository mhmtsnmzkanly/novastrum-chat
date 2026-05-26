use serde::{Deserialize, Serialize};

use crate::auth::dto::PublicUserResponse;

#[derive(Debug, Deserialize)]
pub struct CreateDirectConversationRequest {
    pub target_user_name: String,
}

#[derive(Debug, Serialize)]
pub struct DirectConversationResponse {
    pub conversation: ConversationResponse,
}

#[derive(Debug, Serialize)]
pub struct ConversationResponse {
    pub public_id: String,
    pub kind: &'static str,
    pub title: Option<String>,
    pub target_user: PublicUserResponse,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageHistoryQuery {
    pub before: Option<String>,
    pub limit: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub message: MessageResponse,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub public_id: String,
    pub conversation_id: String,
    pub sender: MessageSenderResponse,
    pub body: String,
    pub message_type: &'static str,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct MessageSenderResponse {
    pub public_id: String,
    pub user_name: String,
    pub public_name: String,
}

#[derive(Debug, Serialize)]
pub struct MessageHistoryResponse {
    pub items: Vec<HistoryMessageResponse>,
    pub page: MessageHistoryPage,
}

#[derive(Debug, Serialize)]
pub struct MessageHistoryPage {
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct HistoryMessageResponse {
    pub public_id: String,
    pub conversation_id: String,
    pub sender: MessageSenderResponse,
    pub body: Option<String>,
    pub message_type: &'static str,
    pub created_at: String,
    pub deleted_at: Option<String>,
}
