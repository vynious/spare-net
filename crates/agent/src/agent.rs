use std::{error::Error, sync::Arc};

use tracing::info;

use crate::{
    connection::{send_deal, Deal},
    discovery::{DiscoveryService, PeerInfo},
};

pub struct Agent {
    discovery: Arc<DiscoveryService>,
}

impl Agent {
    pub async fn new(peer_info: PeerInfo) -> Result<Self, Box<dyn Error>> {
        let dsvc = Arc::new(DiscoveryService::new(peer_info).await?);
        Ok(Agent { discovery: dsvc })
    }

    #[cfg(test)]
    pub async fn test_with_addr(
        peer_info: PeerInfo,
        bind_addr: &str,
        dest_addr: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let dsvc =
            Arc::new(DiscoveryService::test_with_addr(peer_info, bind_addr, dest_addr).await?);
        Ok(Agent { discovery: dsvc })
    }

    pub async fn run(self: Arc<Self>) {
        let dsvc = self.discovery.clone();
        let _ = tokio::spawn(async move {
            dsvc.start().await;
        });
    }

    fn deal_match(&self, peer_info: &PeerInfo, deal: &Deal) -> bool {
        (peer_info.spare_mbs >= deal.file_len) && (peer_info.price <= deal.price_per_mb)
    }

    pub async fn match_deals_with_peers(&self, deal: Deal) {
        let peers = self.discovery.get_peers().await;
        peers.iter().for_each(|peer| {
            if self.deal_match(&peer, &deal) {
                info!("Matched deal with peer {}", peer.peer_id);
            }
        });
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
    async fn two_agents_communicate() {
        let peer_info1 = PeerInfo {
            peer_id: PeerId::random(),
            spare_mbs: 13,
            price: 13.0,
        };
        let peer_info2 = PeerInfo {
            peer_id: PeerId::random(),
            spare_mbs: 13,
            price: 13.0,
        };

        let agent1 = Arc::new(
            Agent::test_with_addr(peer_info1.clone(), "127.0.0.1:6100", "127.0.0.1:6102")
                .await
                .unwrap(),
        );
        let agent2 = Arc::new(
            Agent::test_with_addr(peer_info2.clone(), "127.0.0.1:6102", "127.0.0.1:6100")
                .await
                .unwrap(),
        );

        let _ = agent1.clone().run().await;
        let _ = agent2.clone().run().await;

        time::sleep(Duration::from_secs(3)).await;

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
    }
}
