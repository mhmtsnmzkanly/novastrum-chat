#![allow(dead_code)]

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicIdPrefix {
    User,
    Session,
    Conversation,
    ConversationMembership,
    Message,
    Event,
    Community,
    DiscussionPost,
}

impl PublicIdPrefix {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "usr_",
            Self::Session => "ses_",
            Self::Conversation => "cnv_",
            Self::ConversationMembership => "cvm_",
            Self::Message => "msg_",
            Self::Event => "evt_",
            Self::Community => "com_",
            Self::DiscussionPost => "pst_",
        }
    }
}

impl fmt::Display for PublicIdPrefix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub fn generate_public_id(prefix: PublicIdPrefix) -> String {
    // ULIDs keep public IDs compact and sortable without exposing internal DB ids.
    format!(
        "{}{}",
        prefix.as_str(),
        ulid::Ulid::new().to_string().to_lowercase()
    )
}

pub fn generate_user_public_id() -> String {
    generate_public_id(PublicIdPrefix::User)
}

pub fn generate_session_public_id() -> String {
    generate_public_id(PublicIdPrefix::Session)
}

pub fn generate_event_public_id() -> String {
    generate_public_id(PublicIdPrefix::Event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_public_id_has_expected_prefix_and_length() {
        let public_id = generate_user_public_id();

        assert!(public_id.starts_with("usr_"));
        assert_eq!(public_id.len(), 30);
    }

    #[test]
    fn session_public_id_has_expected_prefix_and_length() {
        let public_id = generate_session_public_id();

        assert!(public_id.starts_with("ses_"));
        assert_eq!(public_id.len(), 30);
    }

    #[test]
    fn event_public_id_has_expected_prefix_and_length() {
        let public_id = generate_event_public_id();

        assert!(public_id.starts_with("evt_"));
        assert_eq!(public_id.len(), 30);
    }
}
