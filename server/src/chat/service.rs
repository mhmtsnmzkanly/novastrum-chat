use std::collections::BTreeMap;

use crate::{
    app_state::AppState,
    auth::{
        current_user::AuthenticatedUser,
        dto::PublicUserResponse,
        service::{is_valid_user_name, normalize_user_name},
    },
    chat::{
        dto::{
            ConversationLatestMessageResponse, ConversationListItemResponse, ConversationListPage,
            ConversationListQuery, ConversationListResponse, ConversationListUserResponse,
            ConversationResponse, CreateDirectConversationRequest, DirectConversationResponse,
            HistoryMessageResponse, MessageHistoryPage, MessageHistoryQuery,
            MessageHistoryResponse, MessageResponse, MessageSenderResponse, SendMessageRequest,
            SendMessageResponse,
        },
        model::{canonical_direct_pair, MembershipRole, MembershipStatus, MessageType},
        repository::{
            ChatConversation, ChatConversationListItem, ChatHistoryMessage, ChatMessage,
            ChatRepository, ChatRepositoryError, ChatUser, NewConversationMembership,
            NewDirectConversation, NewMessage,
        },
    },
    public_id::{generate_public_id, PublicIdPrefix},
    users::model::{DmPolicy, UserStatus},
};

const MAX_MESSAGE_BODY_CHARS: usize = 4_000;
const DEFAULT_HISTORY_LIMIT: u32 = 50;
const MAX_HISTORY_LIMIT: u32 = 100;
const DEFAULT_CONVERSATION_LIST_LIMIT: u32 = 50;
const MAX_CONVERSATION_LIST_LIMIT: u32 = 100;

pub struct ChatService<'a> {
    state: &'a AppState,
}

impl<'a> ChatService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn create_or_get_direct_conversation(
        &self,
        requester: AuthenticatedUser,
        request: CreateDirectConversationRequest,
    ) -> Result<DirectConversationResponse, ChatServiceError> {
        ensure_current_user_can_write(requester.status)?;
        let target_user_name = validate_target_user_name(request.target_user_name)?;
        if target_user_name == requester.user_name {
            return Err(ChatServiceError::CannotMessageSelf);
        }

        let pool = self.state.database.pool().ok_or_else(|| {
            ChatServiceError::DatabaseUnavailable(
                self.state
                    .database
                    .unavailable_reason()
                    .unwrap_or("Database pool is not available")
                    .to_string(),
            )
        })?;
        let repository = ChatRepository::new(pool);
        let target = repository
            .find_user_by_user_name(&target_user_name)
            .await
            .map_err(ChatServiceError::from)?
            .ok_or(ChatServiceError::UserNotFound)?;

        ensure_target_can_receive_direct_message(target.status)?;
        enforce_dm_policy(&repository, requester.id, &target).await?;

        let (user_low_id, user_high_id) = canonical_direct_pair(requester.id, target.id)
            .ok_or(ChatServiceError::CannotMessageSelf)?;

        let conversation = match repository
            .find_direct_conversation_by_pair(user_low_id, user_high_id)
            .await
            .map_err(ChatServiceError::from)?
        {
            Some(conversation) => conversation,
            None => {
                let new_conversation = NewDirectConversation {
                    public_id: generate_public_id(PublicIdPrefix::Conversation),
                    created_by: requester.id,
                    user_low_id,
                    user_high_id,
                    current_membership: active_member(requester.id),
                    target_membership: active_member(target.id),
                };

                repository
                    .create_direct_conversation(new_conversation)
                    .await
                    .map_err(ChatServiceError::from)?
            }
        };

        Ok(to_direct_conversation_response(conversation, target))
    }

    pub async fn send_message(
        &self,
        requester: AuthenticatedUser,
        conversation_public_id: String,
        request: SendMessageRequest,
    ) -> Result<SendMessageResponse, ChatServiceError> {
        ensure_current_user_can_write(requester.status)?;
        let body = validate_message_body(request.body)?;

        let pool = self.state.database.pool().ok_or_else(|| {
            ChatServiceError::DatabaseUnavailable(
                self.state
                    .database
                    .unavailable_reason()
                    .unwrap_or("Database pool is not available")
                    .to_string(),
            )
        })?;
        let repository = ChatRepository::new(pool);
        let conversation = repository
            .find_conversation_by_public_id(&conversation_public_id)
            .await
            .map_err(ChatServiceError::from)?
            .ok_or(ChatServiceError::ConversationNotFound)?;

        repository
            .find_active_membership(conversation.id, requester.id)
            .await
            .map_err(ChatServiceError::from)?
            .ok_or(ChatServiceError::NotConversationMember)?;

        let message = repository
            .insert_message(NewMessage {
                public_id: generate_public_id(PublicIdPrefix::Message),
                conversation_id: conversation.id,
                sender_id: requester.id,
                body,
                message_type: MessageType::Text,
            })
            .await
            .map_err(ChatServiceError::from)?;

        Ok(to_send_message_response(message))
    }

    pub async fn list_messages(
        &self,
        requester: AuthenticatedUser,
        conversation_public_id: String,
        query: MessageHistoryQuery,
    ) -> Result<MessageHistoryResponse, ChatServiceError> {
        let limit = parse_history_limit(query.limit)?;

        let pool = self.state.database.pool().ok_or_else(|| {
            ChatServiceError::DatabaseUnavailable(
                self.state
                    .database
                    .unavailable_reason()
                    .unwrap_or("Database pool is not available")
                    .to_string(),
            )
        })?;
        let repository = ChatRepository::new(pool);
        let conversation = repository
            .find_conversation_by_public_id(&conversation_public_id)
            .await
            .map_err(ChatServiceError::from)?
            .ok_or(ChatServiceError::ConversationNotFound)?;

        let membership = repository
            .find_active_membership(conversation.id, requester.id)
            .await
            .map_err(ChatServiceError::from)?
            .ok_or(ChatServiceError::NotConversationMember)?;

        let before_message_id = match query.before {
            Some(before) if !before.trim().is_empty() => Some(
                repository
                    .find_message_cursor_by_public_id(conversation.id, before.trim())
                    .await
                    .map_err(ChatServiceError::from)?
                    .ok_or(ChatServiceError::MessageCursorNotFound)?,
            ),
            Some(_) => return Err(validation_error("before", "must not be empty")),
            None => None,
        };
        if before_message_id
            .is_some_and(|message_id| message_id <= membership.visible_from_message_id)
        {
            return Err(ChatServiceError::MessageCursorNotFound);
        }

        let mut messages = repository
            .list_visible_messages(
                conversation.id,
                membership.visible_from_message_id,
                before_message_id,
                limit + 1,
            )
            .await
            .map_err(ChatServiceError::from)?;

        let has_more = messages.len() > limit as usize;
        if has_more {
            messages.truncate(limit as usize);
        }
        messages.reverse();

        let next_cursor = if has_more {
            messages.first().map(|message| message.public_id.clone())
        } else {
            None
        };
        let items = messages
            .into_iter()
            .map(to_history_message_response)
            .collect();

        Ok(MessageHistoryResponse {
            items,
            page: MessageHistoryPage {
                next_cursor,
                has_more,
            },
        })
    }

    pub async fn list_conversations(
        &self,
        requester: AuthenticatedUser,
        query: ConversationListQuery,
    ) -> Result<ConversationListResponse, ChatServiceError> {
        let limit = parse_conversation_list_limit(query.limit)?;
        let kind = parse_conversation_kind_filter(query.kind)?;
        if query.before.is_some_and(|before| !before.trim().is_empty()) {
            return Err(validation_error(
                "before",
                "conversation cursor pagination is deferred",
            ));
        }

        let pool = self.state.database.pool().ok_or_else(|| {
            ChatServiceError::DatabaseUnavailable(
                self.state
                    .database
                    .unavailable_reason()
                    .unwrap_or("Database pool is not available")
                    .to_string(),
            )
        })?;
        let repository = ChatRepository::new(pool);
        let conversations = repository
            .list_conversations_for_user(requester.id, kind, limit)
            .await
            .map_err(ChatServiceError::from)?;

        let mut items = Vec::with_capacity(conversations.len());
        for conversation in conversations {
            items.push(
                to_conversation_list_item_response(&repository, requester.id, conversation).await?,
            );
        }

        Ok(ConversationListResponse {
            items,
            page: ConversationListPage {
                next_cursor: None,
                has_more: false,
            },
        })
    }
}

#[derive(Debug)]
pub enum ChatServiceError {
    Validation(BTreeMap<&'static str, String>),
    CurrentUserCannotWrite,
    CannotMessageSelf,
    UserNotFound,
    ConversationNotFound,
    NotConversationMember,
    MessageCursorNotFound,
    TargetUnavailable,
    DmNotAllowed,
    DatabaseUnavailable(String),
    Conflict,
}

impl From<ChatRepositoryError> for ChatServiceError {
    fn from(error: ChatRepositoryError) -> Self {
        match error {
            ChatRepositoryError::Database => {
                Self::DatabaseUnavailable("Database operation failed".to_string())
            }
            ChatRepositoryError::Conflict => Self::Conflict,
            ChatRepositoryError::NotFound => Self::UserNotFound,
        }
    }
}

pub fn validate_target_user_name(target_user_name: String) -> Result<String, ChatServiceError> {
    let mut fields = BTreeMap::new();
    let target_user_name = normalize_user_name(&target_user_name);

    if target_user_name.is_empty() {
        fields.insert("target_user_name", "required".to_string());
    } else if !(3..=32).contains(&target_user_name.len()) {
        fields.insert("target_user_name", "must be 3 to 32 characters".to_string());
    } else if !is_valid_user_name(&target_user_name) {
        fields.insert(
            "target_user_name",
            "must contain only a-z, 0-9, and underscore".to_string(),
        );
    }

    if fields.is_empty() {
        Ok(target_user_name)
    } else {
        Err(ChatServiceError::Validation(fields))
    }
}

pub fn validate_message_body(body: String) -> Result<String, ChatServiceError> {
    let mut fields = BTreeMap::new();
    let body = body.trim().to_string();

    if body.is_empty() {
        fields.insert("body", "required".to_string());
    } else if body.chars().count() > MAX_MESSAGE_BODY_CHARS {
        fields.insert(
            "body",
            format!("must be {MAX_MESSAGE_BODY_CHARS} characters or fewer"),
        );
    }

    if fields.is_empty() {
        Ok(body)
    } else {
        Err(ChatServiceError::Validation(fields))
    }
}

pub fn parse_history_limit(limit: Option<String>) -> Result<u32, ChatServiceError> {
    let Some(limit) = limit else {
        return Ok(DEFAULT_HISTORY_LIMIT);
    };
    let limit = limit.trim();
    if limit.is_empty() {
        return Err(validation_error("limit", "must be a number"));
    }

    match limit.parse::<u32>() {
        Ok(0) => Err(validation_error("limit", "must be at least 1")),
        Ok(value) => Ok(value.min(MAX_HISTORY_LIMIT)),
        Err(_) => Err(validation_error("limit", "must be a number")),
    }
}

pub fn parse_conversation_list_limit(limit: Option<String>) -> Result<u32, ChatServiceError> {
    let Some(limit) = limit else {
        return Ok(DEFAULT_CONVERSATION_LIST_LIMIT);
    };
    let limit = limit.trim();
    if limit.is_empty() {
        return Err(validation_error("limit", "must be a number"));
    }

    match limit.parse::<u32>() {
        Ok(0) => Err(validation_error("limit", "must be at least 1")),
        Ok(value) => Ok(value.min(MAX_CONVERSATION_LIST_LIMIT)),
        Err(_) => Err(validation_error("limit", "must be a number")),
    }
}

pub fn parse_conversation_kind_filter(
    kind: Option<String>,
) -> Result<Option<crate::chat::model::ConversationKind>, ChatServiceError> {
    let Some(kind) = kind else {
        return Ok(None);
    };
    let kind = kind.trim();
    if kind.is_empty() {
        return Err(validation_error("kind", "must be direct or group"));
    }

    kind.parse()
        .map(Some)
        .map_err(|_| validation_error("kind", "must be direct or group"))
}

fn validation_error(field: &'static str, message: impl Into<String>) -> ChatServiceError {
    let mut fields = BTreeMap::new();
    fields.insert(field, message.into());
    ChatServiceError::Validation(fields)
}

pub fn ensure_current_user_can_write(status: UserStatus) -> Result<(), ChatServiceError> {
    match status {
        UserStatus::Active => Ok(()),
        UserStatus::Suspended | UserStatus::Pending | UserStatus::Banned | UserStatus::Deleted => {
            Err(ChatServiceError::CurrentUserCannotWrite)
        }
    }
}

pub fn ensure_target_can_receive_direct_message(
    status: UserStatus,
) -> Result<(), ChatServiceError> {
    match status {
        UserStatus::Active | UserStatus::Suspended => Ok(()),
        UserStatus::Pending | UserStatus::Banned | UserStatus::Deleted => {
            Err(ChatServiceError::TargetUnavailable)
        }
    }
}

pub async fn enforce_dm_policy(
    repository: &ChatRepository<'_>,
    requester_id: u64,
    target: &ChatUser,
) -> Result<(), ChatServiceError> {
    match dm_policy_decision(target.dm_policy) {
        DmPolicyDecision::Allow => Ok(()),
        DmPolicyDecision::RequireSharedGroup => {
            if repository
                .users_share_active_group(requester_id, target.id)
                .await
                .map_err(ChatServiceError::from)?
            {
                Ok(())
            } else {
                Err(ChatServiceError::DmNotAllowed)
            }
        }
        DmPolicyDecision::Deny => Err(ChatServiceError::DmNotAllowed),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmPolicyDecision {
    Allow,
    RequireSharedGroup,
    Deny,
}

pub fn dm_policy_decision(policy: DmPolicy) -> DmPolicyDecision {
    match policy {
        DmPolicy::Everyone => DmPolicyDecision::Allow,
        DmPolicy::SharedGroupMembers => DmPolicyDecision::RequireSharedGroup,
        DmPolicy::FriendsOnly | DmPolicy::None => DmPolicyDecision::Deny,
    }
}

fn active_member(user_id: u64) -> NewConversationMembership {
    NewConversationMembership {
        public_id: generate_public_id(PublicIdPrefix::ConversationMembership),
        conversation_id: 0,
        user_id,
        role: MembershipRole::Member,
        status: MembershipStatus::Active,
        visible_from_message_id: 0,
        removed_by: None,
    }
}

fn to_direct_conversation_response(
    conversation: ChatConversation,
    target: ChatUser,
) -> DirectConversationResponse {
    DirectConversationResponse {
        conversation: ConversationResponse {
            public_id: conversation.public_id,
            kind: conversation.kind.as_str(),
            title: conversation.title,
            target_user: PublicUserResponse {
                public_id: target.public_id,
                user_name: target.user_name,
                public_name: target.public_name,
                status: target.status,
                dm_policy: target.dm_policy,
            },
        },
    }
}

fn to_send_message_response(message: ChatMessage) -> SendMessageResponse {
    SendMessageResponse {
        message: MessageResponse {
            public_id: message.public_id,
            conversation_id: message.conversation_public_id,
            sender: MessageSenderResponse {
                public_id: message.sender_public_id,
                user_name: message.sender_user_name,
                public_name: message.sender_public_name,
            },
            body: message.body,
            message_type: message.message_type.as_str(),
            created_at: message.created_at,
        },
    }
}

fn to_history_message_response(message: ChatHistoryMessage) -> HistoryMessageResponse {
    HistoryMessageResponse {
        public_id: message.public_id,
        conversation_id: message.conversation_public_id,
        sender: MessageSenderResponse {
            public_id: message.sender_public_id,
            user_name: message.sender_user_name,
            public_name: message.sender_public_name,
        },
        body: message.body,
        message_type: message.message_type.as_str(),
        created_at: message.created_at,
        deleted_at: message.deleted_at,
    }
}

async fn to_conversation_list_item_response(
    repository: &ChatRepository<'_>,
    current_user_id: u64,
    conversation: ChatConversationListItem,
) -> Result<ConversationListItemResponse, ChatServiceError> {
    let target_user = if conversation.kind == crate::chat::model::ConversationKind::Direct {
        repository
            .find_direct_target_user(conversation.id, current_user_id)
            .await
            .map_err(ChatServiceError::from)?
            .map(to_conversation_list_user_response)
    } else {
        None
    };

    let member_count = if conversation.kind == crate::chat::model::ConversationKind::Group {
        Some(
            repository
                .count_active_members(conversation.id)
                .await
                .map_err(ChatServiceError::from)?,
        )
    } else {
        None
    };

    let latest_message = repository
        .find_latest_visible_message(conversation.id, conversation.visible_from_message_id)
        .await
        .map_err(ChatServiceError::from)?
        .map(to_latest_message_response);

    Ok(ConversationListItemResponse {
        public_id: conversation.public_id,
        kind: conversation.kind.as_str(),
        title: conversation.title,
        target_user,
        member_count,
        latest_message,
    })
}

fn to_conversation_list_user_response(user: ChatUser) -> ConversationListUserResponse {
    ConversationListUserResponse {
        public_id: user.public_id,
        user_name: user.user_name,
        public_name: user.public_name,
        status: user.status,
    }
}

fn to_latest_message_response(message: ChatHistoryMessage) -> ConversationLatestMessageResponse {
    ConversationLatestMessageResponse {
        public_id: message.public_id,
        body: message.body,
        created_at: message.created_at,
        sender: MessageSenderResponse {
            public_id: message.sender_public_id,
            user_name: message.sender_user_name,
            public_name: message.sender_public_name,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_target_user_name() {
        assert_eq!(
            validate_target_user_name(" Ayse_01 ".to_string()).unwrap(),
            "ayse_01"
        );
        assert!(matches!(
            validate_target_user_name("ay".to_string()),
            Err(ChatServiceError::Validation(_))
        ));
        assert!(matches!(
            validate_target_user_name("ayse-01".to_string()),
            Err(ChatServiceError::Validation(_))
        ));
    }

    #[test]
    fn suspended_current_user_cannot_create_conversation() {
        assert!(ensure_current_user_can_write(UserStatus::Active).is_ok());
        assert!(matches!(
            ensure_current_user_can_write(UserStatus::Suspended),
            Err(ChatServiceError::CurrentUserCannotWrite)
        ));
    }

    #[test]
    fn target_status_policy_allows_active_and_suspended_only() {
        assert!(ensure_target_can_receive_direct_message(UserStatus::Active).is_ok());
        assert!(ensure_target_can_receive_direct_message(UserStatus::Suspended).is_ok());
        assert!(matches!(
            ensure_target_can_receive_direct_message(UserStatus::Pending),
            Err(ChatServiceError::TargetUnavailable)
        ));
    }

    #[test]
    fn dm_policy_everyone_allows() {
        assert_eq!(
            dm_policy_decision(DmPolicy::Everyone),
            DmPolicyDecision::Allow
        );
    }

    #[test]
    fn dm_policy_none_rejects() {
        assert_eq!(dm_policy_decision(DmPolicy::None), DmPolicyDecision::Deny);
    }

    #[test]
    fn dm_policy_friends_only_rejects_until_friendships_exist() {
        assert_eq!(
            dm_policy_decision(DmPolicy::FriendsOnly),
            DmPolicyDecision::Deny
        );
    }

    #[test]
    fn dm_policy_shared_group_requires_group_proof() {
        assert_eq!(
            dm_policy_decision(DmPolicy::SharedGroupMembers),
            DmPolicyDecision::RequireSharedGroup
        );
    }

    #[test]
    fn direct_pair_helper_rejects_self() {
        assert_eq!(canonical_direct_pair(1, 1), None);
        assert_eq!(canonical_direct_pair(9, 2), Some((2, 9)));
    }

    #[test]
    fn validates_message_body() {
        assert_eq!(
            validate_message_body("  Merhaba  ".to_string()).unwrap(),
            "Merhaba"
        );
        assert!(matches!(
            validate_message_body("   ".to_string()),
            Err(ChatServiceError::Validation(_))
        ));
        assert!(matches!(
            validate_message_body("a".repeat(MAX_MESSAGE_BODY_CHARS + 1)),
            Err(ChatServiceError::Validation(_))
        ));
    }

    #[test]
    fn parses_history_limit_with_default_and_cap() {
        assert_eq!(parse_history_limit(None).unwrap(), DEFAULT_HISTORY_LIMIT);
        assert_eq!(parse_history_limit(Some("25".to_string())).unwrap(), 25);
        assert_eq!(
            parse_history_limit(Some("500".to_string())).unwrap(),
            MAX_HISTORY_LIMIT
        );
        assert!(matches!(
            parse_history_limit(Some("0".to_string())),
            Err(ChatServiceError::Validation(_))
        ));
        assert!(matches!(
            parse_history_limit(Some("abc".to_string())),
            Err(ChatServiceError::Validation(_))
        ));
    }

    #[test]
    fn parses_conversation_list_limit_with_default_and_cap() {
        assert_eq!(
            parse_conversation_list_limit(None).unwrap(),
            DEFAULT_CONVERSATION_LIST_LIMIT
        );
        assert_eq!(
            parse_conversation_list_limit(Some("25".to_string())).unwrap(),
            25
        );
        assert_eq!(
            parse_conversation_list_limit(Some("500".to_string())).unwrap(),
            MAX_CONVERSATION_LIST_LIMIT
        );
        assert!(matches!(
            parse_conversation_list_limit(Some("0".to_string())),
            Err(ChatServiceError::Validation(_))
        ));
    }

    #[test]
    fn parses_conversation_kind_filter() {
        assert_eq!(parse_conversation_kind_filter(None).unwrap(), None);
        assert_eq!(
            parse_conversation_kind_filter(Some("direct".to_string())).unwrap(),
            Some(crate::chat::model::ConversationKind::Direct)
        );
        assert_eq!(
            parse_conversation_kind_filter(Some("group".to_string())).unwrap(),
            Some(crate::chat::model::ConversationKind::Group)
        );
        assert!(matches!(
            parse_conversation_kind_filter(Some("public".to_string())),
            Err(ChatServiceError::Validation(_))
        ));
    }
}
