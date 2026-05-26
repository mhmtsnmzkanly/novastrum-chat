use serde::{Deserialize, Serialize};

use crate::{
    auth::current_user::AuthenticatedUser, public_id::generate_event_public_id,
    users::model::UserStatus,
};

#[derive(Debug, Deserialize)]
pub struct ClientPacket {
    #[allow(dead_code)]
    pub req: Option<String>,
    #[serde(rename = "type")]
    pub packet_type: String,
    #[allow(dead_code)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PacketState {
    Ok,
    Error,
}

#[derive(Debug, Serialize)]
pub struct ServerPacket<T>
where
    T: Serialize,
{
    pub id: String,
    #[serde(rename = "type")]
    pub packet_type: &'static str,
    pub state: PacketState,
    pub data: T,
}

impl<T> ServerPacket<T>
where
    T: Serialize,
{
    pub fn ok(packet_type: &'static str, data: T) -> Self {
        Self {
            id: generate_event_public_id(),
            packet_type,
            state: PacketState::Ok,
            data,
        }
    }

    pub fn error(packet_type: &'static str, data: T) -> Self {
        Self {
            id: generate_event_public_id(),
            packet_type,
            state: PacketState::Error,
            data,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConnectedData {
    pub user: ConnectedUser,
}

#[derive(Debug, Serialize)]
pub struct ConnectedUser {
    pub public_id: String,
    pub user_name: String,
    pub public_name: String,
    pub status: UserStatus,
}

#[derive(Debug, Serialize)]
pub struct ErrorData {
    pub code: &'static str,
    pub message: &'static str,
}

pub fn connected_packet(user: AuthenticatedUser) -> ServerPacket<ConnectedData> {
    ServerPacket::ok(
        "system.connected",
        ConnectedData {
            user: ConnectedUser {
                public_id: user.public_id,
                user_name: user.user_name,
                public_name: user.public_name,
                status: user.status,
            },
        },
    )
}

pub fn unknown_packet_type_packet() -> ServerPacket<ErrorData> {
    ServerPacket::error(
        "error.validation",
        ErrorData {
            code: "unknown_packet_type",
            message: "Unknown packet type",
        },
    )
}
