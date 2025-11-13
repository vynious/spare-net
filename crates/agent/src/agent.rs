use std::error::Error;

use crate::discovery::{DiscoveryService, PeerInfo};

struct Agent {
    discovery: DiscoveryService,
}

impl Agent {
    pub async fn new(peer_info: PeerInfo) -> Result<Self, Box<dyn Error>> {
        let dsvc = DiscoveryService::new(peer_info).await?;
        Ok(Agent { discovery: dsvc })
    }
}
