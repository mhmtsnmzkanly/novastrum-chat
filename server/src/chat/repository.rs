#![allow(dead_code)]

use sqlx::{MySql, MySqlPool, Row, Transaction};

use crate::{
    chat::model::{ConversationKind, MembershipRole, MembershipStatus, MessageType},
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

    pub async fn find_conversation_by_public_id(
        &self,
        public_id: &str,
    ) -> Result<Option<ChatConversation>, ChatRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, public_id, kind, title
            FROM conversations
            WHERE public_id = ?
                AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(public_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "conversation lookup failed");
            ChatRepositoryError::Database
        })?;

        row.map(chat_conversation_from_row).transpose()
    }

    pub async fn find_active_membership(
        &self,
        conversation_id: u64,
        user_id: u64,
    ) -> Result<Option<ChatMembership>, ChatRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, conversation_id, user_id, role, status, visible_from_message_id
            FROM conversation_memberships
            WHERE conversation_id = ?
                AND user_id = ?
                AND status = 'active'
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(conversation_id)
        .bind(user_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "active membership lookup failed");
            ChatRepositoryError::Database
        })?;

        row.map(chat_membership_from_row).transpose()
    }

    pub async fn insert_message(
        &self,
        new_message: NewMessage,
    ) -> Result<ChatMessage, ChatRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            tracing::warn!(%error, "send message transaction begin failed");
            ChatRepositoryError::Database
        })?;

        let message_result = sqlx::query(
            r#"
            INSERT INTO messages (
                public_id, conversation_id, sender_id, body, message_type, created_at
            )
            VALUES (?, ?, ?, ?, ?, UTC_TIMESTAMP(6))
            "#,
        )
        .bind(&new_message.public_id)
        .bind(new_message.conversation_id)
        .bind(new_message.sender_id)
        .bind(&new_message.body)
        .bind(new_message.message_type.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "message insert failed");
            ChatRepositoryError::Database
        })?;
        let message_id = message_result.last_insert_id();

        sqlx::query(
            r#"
            UPDATE conversations
            SET updated_at = UTC_TIMESTAMP(6)
            WHERE id = ?
            "#,
        )
        .bind(new_message.conversation_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "conversation updated_at touch failed");
            ChatRepositoryError::Database
        })?;

        let row = sqlx::query(
            r#"
            SELECT
                messages.id,
                messages.public_id,
                messages.conversation_id,
                conversations.public_id AS conversation_public_id,
                messages.sender_id,
                users.public_id AS sender_public_id,
                users.user_name AS sender_user_name,
                users.public_name AS sender_public_name,
                messages.body,
                messages.message_type,
                DATE_FORMAT(messages.created_at, '%Y-%m-%dT%H:%i:%s.%fZ') AS created_at
            FROM messages
            INNER JOIN conversations
                ON conversations.id = messages.conversation_id
            INNER JOIN users
                ON users.id = messages.sender_id
            WHERE messages.id = ?
            LIMIT 1
            "#,
        )
        .bind(message_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "inserted message fetch failed");
            ChatRepositoryError::Database
        })?;

        tx.commit().await.map_err(|error| {
            tracing::warn!(%error, "send message transaction commit failed");
            ChatRepositoryError::Database
        })?;

        chat_message_from_row(row)
    }

    pub async fn find_message_cursor_by_public_id(
        &self,
        conversation_id: u64,
        public_id: &str,
    ) -> Result<Option<u64>, ChatRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id
            FROM messages
            WHERE conversation_id = ?
                AND public_id = ?
            LIMIT 1
            "#,
        )
        .bind(conversation_id)
        .bind(public_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "message cursor lookup failed");
            ChatRepositoryError::Database
        })?;

        Ok(row.map(|row| row.get("id")))
    }

    pub async fn list_visible_messages(
        &self,
        conversation_id: u64,
        visible_after_message_id: u64,
        before_message_id: Option<u64>,
        limit: u32,
    ) -> Result<Vec<ChatHistoryMessage>, ChatRepositoryError> {
        let rows = match before_message_id {
            Some(before_message_id) => {
                let query = history_messages_query(
                    "AND messages.id < ?",
                    "ORDER BY messages.id DESC LIMIT ?",
                );
                sqlx::query(&query)
                    .bind(conversation_id)
                    .bind(visible_after_message_id)
                    .bind(before_message_id)
                    .bind(limit)
                    .fetch_all(self.pool)
                    .await
            }
            None => {
                let query = history_messages_query("", "ORDER BY messages.id DESC LIMIT ?");
                sqlx::query(&query)
                    .bind(conversation_id)
                    .bind(visible_after_message_id)
                    .bind(limit)
                    .fetch_all(self.pool)
                    .await
            }
        }
        .map_err(|error| {
            tracing::warn!(%error, "visible message history lookup failed");
            ChatRepositoryError::Database
        })?;

        rows.into_iter()
            .map(chat_history_message_from_row)
            .collect()
    }

    pub async fn list_conversations_for_user(
        &self,
        user_id: u64,
        kind: Option<ConversationKind>,
        limit: u32,
    ) -> Result<Vec<ChatConversationListItem>, ChatRepositoryError> {
        let rows = match kind {
            Some(kind) => {
                sqlx::query(
                    r#"
                    SELECT
                        conversations.id,
                        conversations.public_id,
                        conversations.kind,
                        conversations.title,
                        conversation_memberships.visible_from_message_id
                    FROM conversation_memberships
                    INNER JOIN conversations
                        ON conversations.id = conversation_memberships.conversation_id
                    WHERE conversation_memberships.user_id = ?
                        AND conversation_memberships.status = 'active'
                        AND conversations.deleted_at IS NULL
                        AND conversations.kind = ?
                    ORDER BY conversations.updated_at DESC, conversations.id DESC
                    LIMIT ?
                    "#,
                )
                .bind(user_id)
                .bind(kind.as_str())
                .bind(limit)
                .fetch_all(self.pool)
                .await
            }
            None => {
                sqlx::query(
                    r#"
                    SELECT
                        conversations.id,
                        conversations.public_id,
                        conversations.kind,
                        conversations.title,
                        conversation_memberships.visible_from_message_id
                    FROM conversation_memberships
                    INNER JOIN conversations
                        ON conversations.id = conversation_memberships.conversation_id
                    WHERE conversation_memberships.user_id = ?
                        AND conversation_memberships.status = 'active'
                        AND conversations.deleted_at IS NULL
                    ORDER BY conversations.updated_at DESC, conversations.id DESC
                    LIMIT ?
                    "#,
                )
                .bind(user_id)
                .bind(limit)
                .fetch_all(self.pool)
                .await
            }
        }
        .map_err(|error| {
            tracing::warn!(%error, "conversation list lookup failed");
            ChatRepositoryError::Database
        })?;

        rows.into_iter()
            .map(chat_conversation_list_item_from_row)
            .collect()
    }

    pub async fn find_direct_target_user(
        &self,
        conversation_id: u64,
        current_user_id: u64,
    ) -> Result<Option<ChatUser>, ChatRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT users.id, users.public_id, users.user_name, users.public_name, users.status, users.dm_policy
            FROM conversation_memberships
            INNER JOIN users
                ON users.id = conversation_memberships.user_id
            WHERE conversation_memberships.conversation_id = ?
                AND conversation_memberships.user_id <> ?
                AND conversation_memberships.status = 'active'
            ORDER BY conversation_memberships.id ASC
            LIMIT 1
            "#,
        )
        .bind(conversation_id)
        .bind(current_user_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "direct target user lookup failed");
            ChatRepositoryError::Database
        })?;

        row.map(chat_user_from_row).transpose()
    }

    pub async fn count_active_members(
        &self,
        conversation_id: u64,
    ) -> Result<u64, ChatRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS member_count
            FROM conversation_memberships
            WHERE conversation_id = ?
                AND status = 'active'
            "#,
        )
        .bind(conversation_id)
        .fetch_one(self.pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "active member count failed");
            ChatRepositoryError::Database
        })?;

        let member_count: i64 = row.get("member_count");
        Ok(member_count.try_into().unwrap_or(0))
    }

    pub async fn find_latest_visible_message(
        &self,
        conversation_id: u64,
        visible_after_message_id: u64,
    ) -> Result<Option<ChatHistoryMessage>, ChatRepositoryError> {
        let query = history_messages_query("", "ORDER BY messages.id DESC LIMIT 1");
        let row = sqlx::query(&query)
            .bind(conversation_id)
            .bind(visible_after_message_id)
            .fetch_optional(self.pool)
            .await
            .map_err(|error| {
                tracing::warn!(%error, "latest visible message lookup failed");
                ChatRepositoryError::Database
            })?;

        row.map(chat_history_message_from_row).transpose()
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
pub struct NewMessage {
    pub public_id: String,
    pub conversation_id: u64,
    pub sender_id: u64,
    pub body: String,
    pub message_type: MessageType,
}

#[derive(Debug)]
pub struct ChatConversation {
    pub id: u64,
    pub public_id: String,
    pub kind: ConversationKind,
    pub title: Option<String>,
}

#[derive(Debug)]
pub struct ChatMembership {
    #[allow(dead_code)]
    pub id: u64,
    #[allow(dead_code)]
    pub conversation_id: u64,
    #[allow(dead_code)]
    pub user_id: u64,
    #[allow(dead_code)]
    pub role: MembershipRole,
    #[allow(dead_code)]
    pub status: MembershipStatus,
    pub visible_from_message_id: u64,
}

#[derive(Debug)]
pub struct ChatMessage {
    #[allow(dead_code)]
    pub id: u64,
    pub public_id: String,
    pub conversation_public_id: String,
    #[allow(dead_code)]
    pub sender_id: u64,
    pub sender_public_id: String,
    pub sender_user_name: String,
    pub sender_public_name: String,
    pub body: String,
    pub message_type: MessageType,
    pub created_at: String,
}

#[derive(Debug)]
pub struct ChatHistoryMessage {
    pub id: u64,
    pub public_id: String,
    pub conversation_public_id: String,
    pub sender_public_id: String,
    pub sender_user_name: String,
    pub sender_public_name: String,
    pub body: Option<String>,
    pub message_type: MessageType,
    pub created_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug)]
pub struct ChatConversationListItem {
    pub id: u64,
    pub public_id: String,
    pub kind: ConversationKind,
    pub title: Option<String>,
    pub visible_from_message_id: u64,
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

fn chat_membership_from_row(
    row: sqlx::mysql::MySqlRow,
) -> Result<ChatMembership, ChatRepositoryError> {
    let role = row
        .get::<String, _>("role")
        .parse::<MembershipRole>()
        .map_err(|error| {
            tracing::warn!(%error, "membership row has invalid role");
            ChatRepositoryError::Database
        })?;
    let status = row
        .get::<String, _>("status")
        .parse::<MembershipStatus>()
        .map_err(|error| {
            tracing::warn!(%error, "membership row has invalid status");
            ChatRepositoryError::Database
        })?;

    Ok(ChatMembership {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        user_id: row.get("user_id"),
        role,
        status,
        visible_from_message_id: row.get("visible_from_message_id"),
    })
}

fn chat_message_from_row(row: sqlx::mysql::MySqlRow) -> Result<ChatMessage, ChatRepositoryError> {
    let message_type = row
        .get::<String, _>("message_type")
        .parse::<MessageType>()
        .map_err(|error| {
            tracing::warn!(%error, "message row has invalid message_type");
            ChatRepositoryError::Database
        })?;

    Ok(ChatMessage {
        id: row.get("id"),
        public_id: row.get("public_id"),
        conversation_public_id: row.get("conversation_public_id"),
        sender_id: row.get("sender_id"),
        sender_public_id: row.get("sender_public_id"),
        sender_user_name: row.get("sender_user_name"),
        sender_public_name: row.get("sender_public_name"),
        body: row.get("body"),
        message_type,
        created_at: row.get("created_at"),
    })
}

fn chat_history_message_from_row(
    row: sqlx::mysql::MySqlRow,
) -> Result<ChatHistoryMessage, ChatRepositoryError> {
    let message_type = row
        .get::<String, _>("message_type")
        .parse::<MessageType>()
        .map_err(|error| {
            tracing::warn!(%error, "history message row has invalid message_type");
            ChatRepositoryError::Database
        })?;

    Ok(ChatHistoryMessage {
        id: row.get("id"),
        public_id: row.get("public_id"),
        conversation_public_id: row.get("conversation_public_id"),
        sender_public_id: row.get("sender_public_id"),
        sender_user_name: row.get("sender_user_name"),
        sender_public_name: row.get("sender_public_name"),
        body: row.get("body"),
        message_type,
        created_at: row.get("created_at"),
        deleted_at: row.get("deleted_at"),
    })
}

fn chat_conversation_list_item_from_row(
    row: sqlx::mysql::MySqlRow,
) -> Result<ChatConversationListItem, ChatRepositoryError> {
    let kind = row
        .get::<String, _>("kind")
        .parse::<ConversationKind>()
        .map_err(|error| {
            tracing::warn!(%error, "conversation list row has invalid kind");
            ChatRepositoryError::Database
        })?;

    Ok(ChatConversationListItem {
        id: row.get("id"),
        public_id: row.get("public_id"),
        kind,
        title: row.get("title"),
        visible_from_message_id: row.get("visible_from_message_id"),
    })
}

fn history_messages_query(extra_filter: &'static str, order_limit: &'static str) -> String {
    format!(
        r#"
        SELECT
            messages.id,
            messages.public_id,
            conversations.public_id AS conversation_public_id,
            users.public_id AS sender_public_id,
            users.user_name AS sender_user_name,
            users.public_name AS sender_public_name,
            CASE
                WHEN messages.deleted_at IS NULL THEN messages.body
                ELSE NULL
            END AS body,
            messages.message_type,
            DATE_FORMAT(messages.created_at, '%Y-%m-%dT%H:%i:%s.%fZ') AS created_at,
            CASE
                WHEN messages.deleted_at IS NULL THEN NULL
                ELSE DATE_FORMAT(messages.deleted_at, '%Y-%m-%dT%H:%i:%s.%fZ')
            END AS deleted_at
        FROM messages
        INNER JOIN conversations
            ON conversations.id = messages.conversation_id
        INNER JOIN users
            ON users.id = messages.sender_id
        WHERE messages.conversation_id = ?
            AND messages.id > ?
            {extra_filter}
        {order_limit}
        "#
    )
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
