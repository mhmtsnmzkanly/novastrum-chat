use std::{
    collections::HashSet,
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use tokio::sync::{mpsc, RwLock};

use crate::ws::protocol::OutboundPacket;

pub type ConnectionId = u64;
pub type UserId = u64;

const OUTBOUND_BUFFER_SIZE: usize = 32;

#[derive(Clone, Debug)]
pub struct WsHub {
    inner: Arc<WsHubInner>,
}

#[derive(Debug)]
struct WsHubInner {
    next_connection_id: AtomicU64,
    connections: RwLock<HashMap<UserId, HashMap<ConnectionId, mpsc::Sender<OutboundPacket>>>>,
}

impl WsHub {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WsHubInner {
                next_connection_id: AtomicU64::new(1),
                connections: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub async fn register(&self, user_id: UserId) -> RegisteredConnection {
        let connection_id = self
            .inner
            .next_connection_id
            .fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel(OUTBOUND_BUFFER_SIZE);

        self.inner
            .connections
            .write()
            .await
            .entry(user_id)
            .or_default()
            .insert(connection_id, sender);

        tracing::debug!(user_id, connection_id, "registered websocket connection");

        RegisteredConnection {
            user_id,
            connection_id,
            receiver,
        }
    }

    pub async fn unregister(&self, user_id: UserId, connection_id: ConnectionId) {
        let mut connections = self.inner.connections.write().await;

        if let Some(user_connections) = connections.get_mut(&user_id) {
            user_connections.remove(&connection_id);
            if user_connections.is_empty() {
                connections.remove(&user_id);
            }
        }

        tracing::debug!(user_id, connection_id, "unregistered websocket connection");
    }

    #[allow(dead_code)]
    pub async fn send_to_user(&self, user_id: UserId, packet: OutboundPacket) -> usize {
        let senders = {
            let connections = self.inner.connections.read().await;
            connections
                .get(&user_id)
                .map(|user_connections| user_connections.values().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        };

        let mut sent = 0;
        for sender in senders {
            if sender.send(packet.clone()).await.is_ok() {
                sent += 1;
            }
        }

        sent
    }

    #[allow(dead_code)]
    pub async fn connection_count_for_user(&self, user_id: UserId) -> usize {
        self.inner
            .connections
            .read()
            .await
            .get(&user_id)
            .map_or(0, HashMap::len)
    }

    #[allow(dead_code)]
    pub async fn total_connection_count(&self) -> usize {
        self.inner
            .connections
            .read()
            .await
            .values()
            .map(HashMap::len)
            .sum()
    }
}

impl Default for WsHub {
    fn default() -> Self {
        Self::new()
    }
}

impl WsHub {
    /// Broadcast an outbound packet to all active connections across all users.
    pub async fn broadcast(&self, packet: OutboundPacket) -> usize {
        let senders = {
            let connections = self.inner.connections.read().await;
            connections
                .values()
                .flat_map(|user_map| user_map.values().cloned())
                .collect::<Vec<_>>()
        };
        let mut sent = 0;
        for sender in senders {
            if sender.send(packet.clone()).await.is_ok() {
                sent += 1;
            }
        }
        sent
    }

    /// Broadcast packet only to participants whose public_id (as string) is present in the set.
    pub async fn broadcast_to_room(&self, room_participants: &HashSet<String>, packet: OutboundPacket) -> usize {
        // Snapshot senders for matching participants.
        let senders = {
            let connections = self.inner.connections.read().await;
            connections
                .iter()
                .filter_map(|(user_id, user_conns)| {
                    // Convert user_id to string for comparison. Adjust conversion as needed.
                    if room_participants.contains(&user_id.to_string()) {
                        Some(user_conns.values().cloned().collect::<Vec<_>>())
                    } else {
                        None
                    }
                })
                .flatten()
                .collect::<Vec<_>>()
        };
        let mut sent = 0;
        for sender in senders {
            if sender.send(packet.clone()).await.is_ok() {
                sent += 1;
            }
        }
        sent
    }
}


#[derive(Debug)]
pub struct RegisteredConnection {
    #[allow(dead_code)]
    pub user_id: UserId,
    pub connection_id: ConnectionId,
    pub receiver: mpsc::Receiver<OutboundPacket>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws::protocol::{packet_to_outbound, ErrorData, ServerPacket};

    #[tokio::test]
    async fn register_increments_counts() {
        let hub = WsHub::new();

        let _connection = hub.register(7).await;

        assert_eq!(hub.connection_count_for_user(7).await, 1);
        assert_eq!(hub.total_connection_count().await, 1);
    }

    #[tokio::test]
    async fn unregister_decrements_counts() {
        let hub = WsHub::new();
        let connection = hub.register(7).await;

        hub.unregister(connection.user_id, connection.connection_id)
            .await;

        assert_eq!(hub.connection_count_for_user(7).await, 0);
        assert_eq!(hub.total_connection_count().await, 0);
    }

    #[tokio::test]
    async fn supports_multiple_connections_per_user() {
        let hub = WsHub::new();

        let first = hub.register(7).await;
        let _second = hub.register(7).await;
        let _other_user = hub.register(8).await;

        assert_eq!(hub.connection_count_for_user(7).await, 2);
        assert_eq!(hub.total_connection_count().await, 3);

        hub.unregister(first.user_id, first.connection_id).await;

        assert_eq!(hub.connection_count_for_user(7).await, 1);
        assert_eq!(hub.total_connection_count().await, 2);
    }

    #[tokio::test]
    async fn send_to_user_delivers_to_each_connection() {
        let hub = WsHub::new();
        let mut first = hub.register(7).await;
        let mut second = hub.register(7).await;
        let _other_user = hub.register(8).await;
        let packet = ServerPacket::error(
            "error.validation",
            ErrorData {
                code: "test",
                message: "Test",
            },
        );

        let sent = hub
            .send_to_user(7, packet_to_outbound(&packet).unwrap())
            .await;

        assert_eq!(sent, 2);
        assert!(first.receiver.recv().await.is_some());
        assert!(second.receiver.recv().await.is_some());
    }
}
