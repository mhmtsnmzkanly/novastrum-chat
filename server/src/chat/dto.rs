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
