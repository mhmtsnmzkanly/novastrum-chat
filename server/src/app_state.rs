use crate::{config::Config, db::Database, ws::hub::WsHub};
use std::sync::Arc;
use tokio::sync::broadcast;
use crate::ws::protocol::OutboundPacket;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Config,
    pub database: Database,
    /// Shared WebSocket hub wrapped in Arc for cheap cloning across threads
    pub ws_hub: Arc<WsHub>,
    /// Broadcast channel for server‑wide outbound packets
    pub ws_broadcast: broadcast::Sender<OutboundPacket>,
    /// Thread‑safe map of active chat rooms
    pub rooms: Arc<RwLock<std::collections::HashMap<String, RoomState>>>,
}

impl AppState {
    pub fn new(config: Config, database: Database) -> Self {
        // Initialize a bounded broadcast channel for outbound packets
        let (tx, _rx) = broadcast::channel::<OutboundPacket>(64);
        Self {
            config,
            database,
            ws_hub: Arc::new(WsHub::new()),
            ws_broadcast: tx,
            rooms: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Broadcast a packet only to the participants of the given room.
    /// Looks up the room's participant set in RAM and forwards the packet to
    /// the hub's `broadcast_to_room` method.
    pub async fn broadcast_to_room_id(&self, room_id: &str, packet: OutboundPacket) {
        // Read‑only access to the rooms map.
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(room_id) {
            // Forward to hub – it will handle filtering the connections.
            self.ws_hub.broadcast_to_room(&room.participants, packet).await;
        } else {
            tracing::warn!(room_id, "attempted to broadcast to unknown room");
        }
    }

}

/// Simple per‑room state kept in memory.
#[derive(Debug, Default, Clone)]
pub struct RoomState {
    /// Set of participant identifiers (e.g., public IDs).
    pub participants: std::collections::HashSet<String>,
}
