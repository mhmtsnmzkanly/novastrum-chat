use std::collections::BTreeMap;

use crate::{
    app_state::AppState,
    auth::{
        current_user::AuthenticatedUser,
        dto::PublicUserResponse,
        service::{is_valid_user_name, normalize_user_name},
    },
    chat::{
        dto::{ConversationResponse, CreateDirectConversationRequest, DirectConversationResponse},
        model::{canonical_direct_pair, MembershipRole, MembershipStatus},
        repository::{
            ChatConversation, ChatRepository, ChatRepositoryError, ChatUser,
            NewConversationMembership, NewDirectConversation,
        },
    },
    public_id::{generate_public_id, PublicIdPrefix},
    users::model::{DmPolicy, UserStatus},
};

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
}

#[derive(Debug)]
pub enum ChatServiceError {
    Validation(BTreeMap<&'static str, String>),
    CurrentUserCannotWrite,
    CannotMessageSelf,
    UserNotFound,
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
}
