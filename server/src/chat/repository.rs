#![allow(dead_code)]

use sqlx::{MySql, MySqlPool, Row, Transaction};

use crate::{
    chat::model::{ConversationKind, MembershipRole, MembershipStatus},
    users::model::{DmPolicy, UserStatus},
};

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

    pub async fn find_user_by_user_name(
        &self,
        user_name: &str,
    ) -> Result<Option<ChatUser>, ChatRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, public_id, user_name, public_name, status, dm_policy
            FROM users
            WHERE user_name = ?
            LIMIT 1
            "#,
        )
        .bind(user_name)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "chat user lookup failed");
            ChatRepositoryError::Database
        })?;

        row.map(chat_user_from_row).transpose()
    }

    pub async fn users_share_active_group(
        &self,
        user_a_id: u64,
        user_b_id: u64,
    ) -> Result<bool, ChatRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT 1 AS shared_group
            FROM conversation_memberships AS a
            INNER JOIN conversation_memberships AS b
                ON b.conversation_id = a.conversation_id
            INNER JOIN conversations
                ON conversations.id = a.conversation_id
            WHERE a.user_id = ?
                AND b.user_id = ?
                AND a.status = 'active'
                AND b.status = 'active'
                AND conversations.kind = 'group'
                AND conversations.deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(user_a_id)
        .bind(user_b_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared group lookup failed");
            ChatRepositoryError::Database
        })?;

        Ok(row.is_some())
    }

    pub async fn find_direct_conversation_by_pair(
        &self,
        user_low_id: u64,
        user_high_id: u64,
    ) -> Result<Option<ChatConversation>, ChatRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT conversations.id, conversations.public_id, conversations.kind, conversations.title
            FROM direct_conversation_pairs
            INNER JOIN conversations
                ON conversations.id = direct_conversation_pairs.conversation_id
            WHERE direct_conversation_pairs.user_low_id = ?
                AND direct_conversation_pairs.user_high_id = ?
                AND conversations.deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(user_low_id)
        .bind(user_high_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "direct conversation lookup failed");
            ChatRepositoryError::Database
        })?;

        row.map(chat_conversation_from_row).transpose()
    }

    pub async fn create_direct_conversation(
        &self,
        new_conversation: NewDirectConversation,
    ) -> Result<ChatConversation, ChatRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            tracing::warn!(%error, "direct conversation transaction begin failed");
            ChatRepositoryError::Database
        })?;

        let conversation_result = sqlx::query(
            r#"
            INSERT INTO conversations (
                public_id, kind, title, created_by, created_at, updated_at
            )
            VALUES (?, 'direct', NULL, ?, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))
            "#,
        )
        .bind(&new_conversation.public_id)
        .bind(new_conversation.created_by)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "direct conversation insert failed");
            ChatRepositoryError::Database
        })?;
        let conversation_id = conversation_result.last_insert_id();

        sqlx::query(
            r#"
            INSERT INTO direct_conversation_pairs (
                conversation_id, user_low_id, user_high_id, created_at
            )
            VALUES (?, ?, ?, UTC_TIMESTAMP(6))
            "#,
        )
        .bind(conversation_id)
        .bind(new_conversation.user_low_id)
        .bind(new_conversation.user_high_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "direct conversation pair insert failed");
            if is_duplicate_key(&error) {
                ChatRepositoryError::Conflict
            } else {
                ChatRepositoryError::Database
            }
        })?;

        insert_membership(
            &mut tx,
            conversation_id,
            &new_conversation.current_membership,
        )
        .await?;
        insert_membership(
            &mut tx,
            conversation_id,
            &new_conversation.target_membership,
        )
        .await?;

        tx.commit().await.map_err(|error| {
            tracing::warn!(%error, "direct conversation transaction commit failed");
            ChatRepositoryError::Database
        })?;

        Ok(ChatConversation {
            id: conversation_id,
            public_id: new_conversation.public_id,
            kind: ConversationKind::Direct,
            title: None,
        })
    }
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
pub struct NewDirectConversation {
    pub public_id: String,
    pub created_by: u64,
    pub user_low_id: u64,
    pub user_high_id: u64,
    pub current_membership: NewConversationMembership,
    pub target_membership: NewConversationMembership,
}

#[derive(Debug)]
pub struct ChatConversation {
    #[allow(dead_code)]
    pub id: u64,
    pub public_id: String,
    pub kind: ConversationKind,
    pub title: Option<String>,
}

#[derive(Debug)]
pub struct ChatUser {
    pub id: u64,
    pub public_id: String,
    pub user_name: String,
    pub public_name: String,
    pub status: UserStatus,
    pub dm_policy: DmPolicy,
}

#[derive(Debug)]
pub enum ChatRepositoryError {
    Database,
    Conflict,
    NotFound,
}

fn chat_user_from_row(row: sqlx::mysql::MySqlRow) -> Result<ChatUser, ChatRepositoryError> {
    let status = row
        .get::<String, _>("status")
        .parse::<UserStatus>()
        .map_err(|error| {
            tracing::warn!(%error, "chat user row has invalid status");
            ChatRepositoryError::Database
        })?;
    let dm_policy = row
        .get::<String, _>("dm_policy")
        .parse::<DmPolicy>()
        .map_err(|error| {
            tracing::warn!(%error, "chat user row has invalid dm_policy");
            ChatRepositoryError::Database
        })?;

    Ok(ChatUser {
        id: row.get("id"),
        public_id: row.get("public_id"),
        user_name: row.get("user_name"),
        public_name: row.get("public_name"),
        status,
        dm_policy,
    })
}

fn chat_conversation_from_row(
    row: sqlx::mysql::MySqlRow,
) -> Result<ChatConversation, ChatRepositoryError> {
    let kind = row
        .get::<String, _>("kind")
        .parse::<ConversationKind>()
        .map_err(|error| {
            tracing::warn!(%error, "conversation row has invalid kind");
            ChatRepositoryError::Database
        })?;

    Ok(ChatConversation {
        id: row.get("id"),
        public_id: row.get("public_id"),
        kind,
        title: row.get("title"),
    })
}

async fn insert_membership(
    tx: &mut Transaction<'_, MySql>,
    conversation_id: u64,
    membership: &NewConversationMembership,
) -> Result<(), ChatRepositoryError> {
    sqlx::query(
        r#"
        INSERT INTO conversation_memberships (
            public_id,
            conversation_id,
            user_id,
            role,
            status,
            visible_from_message_id,
            joined_at,
            removed_by
        )
        VALUES (?, ?, ?, ?, ?, ?, UTC_TIMESTAMP(6), ?)
        "#,
    )
    .bind(&membership.public_id)
    .bind(conversation_id)
    .bind(membership.user_id)
    .bind(membership.role.as_str())
    .bind(membership.status.as_str())
    .bind(membership.visible_from_message_id)
    .bind(membership.removed_by)
    .execute(&mut **tx)
    .await
    .map(|_| ())
    .map_err(|error| {
        tracing::warn!(%error, "conversation membership insert failed");
        ChatRepositoryError::Database
    })
}

fn is_duplicate_key(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(database_error) = error else {
        return false;
    };

    database_error.code().as_deref() == Some("1062")
}
