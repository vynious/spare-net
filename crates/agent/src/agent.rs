use futures::future::join_all;
use quinn::Endpoint;
use std::{error::Error, sync::Arc};
use tracing::info;

use crate::{
    connection::{open_receiver_endpoint, open_sender_endpoint, receive, send, Deal},
    discovery::{DiscoveryService, PeerInfo},
};

pub struct Agent {
    discovery: Arc<DiscoveryService>,
    receiver_endpoint: Endpoint,
    sender_endpoint: Endpoint,
}

impl Agent {
    pub async fn new(peer_info: PeerInfo) -> Result<Self, Box<dyn Error>> {
        let listen_addr = peer_info.clone().addr;
        let dsvc = Arc::new(DiscoveryService::new(peer_info).await?);
        let rep = open_receiver_endpoint(listen_addr).await?;
        let sep = open_sender_endpoint().await?;
        Ok(Agent {
            discovery: dsvc,
            receiver_endpoint: rep,
            sender_endpoint: sep,
        })
    }

    #[cfg(test)]
    pub async fn test_with_addr(
        peer_info: PeerInfo,
        bind_addr: &str,
        dest_addr: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let listen_addr = peer_info.clone().addr;
        let dsvc =
            Arc::new(DiscoveryService::test_with_addr(peer_info, bind_addr, dest_addr).await?);
        let ep = open_receiver_endpoint(listen_addr).await?;
        let sep = open_sender_endpoint().await?;

        Ok(Agent {
            discovery: dsvc,
            receiver_endpoint: ep,
            sender_endpoint: sep,
        })
    }

    pub async fn run(self: Arc<Self>) {
        let dsvc = self.discovery.clone();
        let _ = tokio::spawn(async move {
            dsvc.start().await;
        });
        let _ = tokio::spawn(async move {
            self.receive_deals().await;
        });
    }

    fn deal_match(&self, peer_info: &PeerInfo, deal: &Deal) -> bool {
        (peer_info.spare_mbs >= deal.file_len) && (peer_info.price <= deal.price_per_mb)
    }

    pub async fn send_matched_deals(&self, deal: Deal) {
        let peers = self.discovery.get_peers().await;
        let send_tasks = peers.into_iter().map(|peer| {
            let deal = deal.clone();
            let sep = self.sender_endpoint.clone();
            async move {
                if self.deal_match(&peer, &deal) {
                    println!(
                        "Sending matched deal to peer {} at {}",
                        peer.peer_id, peer.addr
                    );
                    if let Err(err) = send(&sep, peer.addr, deal).await {
                        info!("failed to send deal to {}: {err}", peer.peer_id);
                    }
                    println!(
                        "Successfully sent matched deal to peer {} at {}",
                        peer.peer_id, peer.addr
                    );
                }
            }
        });
        join_all(send_tasks).await;
    }

    pub async fn receive_deals(&self) {
        let peer_info = self.get_peer_info().clone();
        println!(
            "Agent {} listening for deals on {}",
            peer_info.peer_id, peer_info.addr
        );
        loop {
            match receive(&self.receiver_endpoint).await {
                Ok(deal) => {
                    println!(
                        "Agent {} received deal: {:?}",
                        self.get_peer_info().peer_id,
                        deal
                    );
                }
                Err(e) => {
                    println!("failed to receive deal: {e}");
                }
            }
        }
    }

    fn get_peer_info(&self) -> &PeerInfo {
        self.discovery.get_peer_info()
    }
}

#[cfg(test)]
mod tests {
    use libp2p::PeerId;
    use std::{sync::Arc, time::Duration};
    use tokio::time;

    use super::*;

    #[tokio::test]
    /// two agents discover each other over loopback sockets
    /// agents will succeed in matching a deal with one another
    /// peer1's deal will be matched with peer2 based on its
    /// `spare_mbs` and `price`.
    async fn two_agents_communicate() {
        let peer_info1 = PeerInfo {
            addr: "127.0.0.1:6101".parse().unwrap(),
            peer_id: PeerId::random(),
            spare_mbs: 14,
            price: 15.0,
        };

        let deal1 = Deal {
            file_len: 40,
            price_per_mb: 10.0,
        };
        let peer_info2 = PeerInfo {
            addr: "127.0.0.1:6103".parse().unwrap(),
            peer_id: PeerId::random(),
            spare_mbs: 50,
            price: 1.0,
        };

        // connect with Agent2's discovery address
        let agent1 = Arc::new(
            Agent::test_with_addr(peer_info1.clone(), "127.0.0.1:6100", "127.0.0.1:6102")
                .await
                .unwrap(),
        );
        // connect with Agent1's discovery address
        let agent2 = Arc::new(
            Agent::test_with_addr(peer_info2.clone(), "127.0.0.1:6102", "127.0.0.1:6100")
                .await
                .unwrap(),
        );

        let _ = tokio::spawn(agent1.clone().run());
        let _ = tokio::spawn(agent2.clone().run());

        time::sleep(Duration::from_secs(2)).await;

        let peers_agent1 = agent1.discovery.get_peers().await;
        let peers_agent2 = agent2.discovery.get_peers().await;

        assert!(
            peers_agent1
                .iter()
                .any(|peer| peer.peer_id == peer_info2.peer_id),
            "agent1 should see agent2"
        );
        assert!(
            peers_agent2
                .iter()
                .any(|peer| peer.peer_id == peer_info1.peer_id),
            "agent2 should see agent1"
        );

        agent1.send_matched_deals(deal1).await;

        time::sleep(Duration::from_secs(1)).await;

        assert_eq!(1, 0);
    }
}
