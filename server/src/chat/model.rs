#![allow(dead_code)]

use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationKind {
    Direct,
    Group,
}

impl ConversationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Group => "group",
        }
    }
}

impl fmt::Display for ConversationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ConversationKind {
    type Err = ChatModelParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "direct" => Ok(Self::Direct),
            "group" => Ok(Self::Group),
            _ => Err(ChatModelParseError::new("conversation kind", value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MembershipRole {
    Owner,
    Member,
}

impl MembershipRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Member => "member",
        }
    }
}

impl fmt::Display for MembershipRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for MembershipRole {
    type Err = ChatModelParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "owner" => Ok(Self::Owner),
            "member" => Ok(Self::Member),
            _ => Err(ChatModelParseError::new("membership role", value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MembershipStatus {
    Active,
    Left,
    Removed,
}

impl MembershipStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Left => "left",
            Self::Removed => "removed",
        }
    }
}

impl fmt::Display for MembershipStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for MembershipStatus {
    type Err = ChatModelParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "left" => Ok(Self::Left),
            "removed" => Ok(Self::Removed),
            _ => Err(ChatModelParseError::new("membership status", value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageType {
    Text,
    System,
}

impl MessageType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::System => "system",
        }
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for MessageType {
    type Err = ChatModelParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "text" => Ok(Self::Text),
            "system" => Ok(Self::System),
            _ => Err(ChatModelParseError::new("message type", value)),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ChatModelParseError {
    field: &'static str,
    value: String,
}

impl ChatModelParseError {
    fn new(field: &'static str, value: &str) -> Self {
        Self {
            field,
            value: value.to_string(),
        }
    }
}

impl fmt::Display for ChatModelParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported {} value `{}`",
            self.field, self.value
        )
    }
}

impl std::error::Error for ChatModelParseError {}

pub fn canonical_direct_pair(user_a_id: u64, user_b_id: u64) -> Option<(u64, u64)> {
    match user_a_id.cmp(&user_b_id) {
        std::cmp::Ordering::Less => Some((user_a_id, user_b_id)),
        std::cmp::Ordering::Greater => Some((user_b_id, user_a_id)),
        std::cmp::Ordering::Equal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_kind_converts_to_and_from_database_value() {
        assert_eq!(ConversationKind::Direct.as_str(), "direct");
        assert_eq!(
            "group".parse::<ConversationKind>().unwrap(),
            ConversationKind::Group
        );
        assert!("channel".parse::<ConversationKind>().is_err());
    }

    #[test]
    fn membership_role_converts_to_and_from_database_value() {
        assert_eq!(MembershipRole::Owner.as_str(), "owner");
        assert_eq!(
            "member".parse::<MembershipRole>().unwrap(),
            MembershipRole::Member
        );
        assert!("admin".parse::<MembershipRole>().is_err());
    }

    #[test]
    fn membership_status_converts_to_and_from_database_value() {
        assert_eq!(MembershipStatus::Active.as_str(), "active");
        assert_eq!(
            "removed".parse::<MembershipStatus>().unwrap(),
            MembershipStatus::Removed
        );
        assert!("blocked".parse::<MembershipStatus>().is_err());
    }

    #[test]
    fn message_type_converts_to_and_from_database_value() {
        assert_eq!(MessageType::Text.as_str(), "text");
        assert_eq!(
            "system".parse::<MessageType>().unwrap(),
            MessageType::System
        );
        assert!("image".parse::<MessageType>().is_err());
    }

    #[test]
    fn canonical_direct_pair_orders_internal_user_ids() {
        assert_eq!(canonical_direct_pair(10, 4), Some((4, 10)));
        assert_eq!(canonical_direct_pair(4, 10), Some((4, 10)));
        assert_eq!(canonical_direct_pair(7, 7), None);
    }
}
