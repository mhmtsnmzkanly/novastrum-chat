#![allow(dead_code)]

use crate::{
    app_state::AppState, auth::current_user::AuthenticatedUser, chat::model::canonical_direct_pair,
};

pub struct ChatService<'a> {
    state: &'a AppState,
}

impl<'a> ChatService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &'a AppState {
        self.state
    }

    pub fn canonical_direct_pair(
        &self,
        requester: &AuthenticatedUser,
        other_user_id: u64,
    ) -> Result<(u64, u64), ChatServiceError> {
        canonical_direct_pair(requester.id, other_user_id)
            .ok_or(ChatServiceError::InvalidDirectPair)
    }

    // Future service responsibilities:
    // - create or return direct conversations inside one transaction
    // - enforce recipient DM policy before new direct conversation creation
    // - enforce group membership and max-size rules
    // - assign visible_from_message_id for added/rejoined group members
    // - apply visible_from_message_id when reading group message history
    // - validate and insert text messages after membership checks
}

#[derive(Debug)]
pub enum ChatServiceError {
    InvalidDirectPair,
    Forbidden,
    GroupFull,
    NotFound,
    Database,
}
