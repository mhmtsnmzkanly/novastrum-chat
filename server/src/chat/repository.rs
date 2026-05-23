#![allow(dead_code)]

use sqlx::MySqlPool;

use crate::chat::model::{MembershipRole, MembershipStatus};

pub struct ChatRepository<'a> {
    pool: &'a MySqlPool,
}

impl<'a> ChatRepository<'a> {
    pub fn new(pool: &'a MySqlPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &'a MySqlPool {
        self.pool
    }

    // Future SQL boundary:
    // - find_direct_pair(user_low_id, user_high_id)
    // - create_direct_conversation(...)
    // - get_latest_message_id(conversation_id)
    // - add_membership(...)
    // - insert_message(...)
    //
    // Keep SQL here, not in HTTP handlers or service orchestration code.
}

#[derive(Debug)]
pub struct NewConversationMembership {
    pub public_id: String,
    pub conversation_id: u64,
    pub user_id: u64,
    pub role: MembershipRole,
    pub status: MembershipStatus,
    pub visible_from_message_id: u64,
    pub removed_by: Option<u64>,
}

#[derive(Debug)]
pub enum ChatRepositoryError {
    Database,
    Conflict,
    NotFound,
}
