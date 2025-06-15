use libp2p::{futures::lock::Mutex, PeerId};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{
    collections::HashMap, error::Error, net::SocketAddr, sync::Arc, time::{Duration, Instant}
};
use tokio::{net::{UdpSocket}, time};

const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(2);
const PEER_TIMEOUT: Duration = Duration::from_secs(5);
const MULTICAST_ADDR: &str = "224.0.0.251:5353";

/// in-memory representation
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub spare_mbs: u16,
    pub price: f32,
}

/// wire representation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfoWire {
    pub peer_id_bytes: ByteBuf,
    pub spare_mbs: u16,
    pub price: f32,
}

/// convert PeerInfo into PeerInfoWire (in-memory -> wire)
impl From<PeerInfo> for PeerInfoWire {
    fn from(pi: PeerInfo) -> Self {
        PeerInfoWire {
            peer_id_bytes: ByteBuf::from(pi.peer_id.to_bytes()),
            spare_mbs: pi.spare_mbs,
            price: pi.price,
        }
    }
}

/// convert PeerInfoWire into PeerInfo (wire -> in-memory)
impl TryFrom<PeerInfoWire> for PeerInfo {
    type Error = libp2p::identity::ParseError;
    fn try_from(w: PeerInfoWire) -> Result<Self, Self::Error> {
        Ok(PeerInfo {
            peer_id: PeerId::from_bytes(&w.peer_id_bytes)?,
            spare_mbs: w.spare_mbs,
            price: w.price,
        })
    }
}

#[derive(Debug)]
pub struct DiscoveryService {
    peers: Arc<Mutex<HashMap<PeerId, (PeerInfo, Instant)>>>,
    socket: Arc<UdpSocket>,
    peer_info: PeerInfo,
    dest: SocketAddr
}

impl DiscoveryService {
    /// creates a new discovery service based on the peer_info
    pub async fn new(peer_info: PeerInfo) -> Result<Self, Box<dyn Error>> {
        Self::with_addr(peer_info, "0.0.0.0:5333", MULTICAST_ADDR).await
    }

    /// test-friendly constructor that binds to specific addresses
    pub async fn with_addr(
        peer_info: PeerInfo,
        bind_addr: &str,
        dest_addr: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        
        let mut parts = dest_addr.split(":");
        let dest_ip: std::net::Ipv4Addr = parts.next().ok_or("missing multicast IP")?.parse()?;
        parts.next().ok_or("missing multicast port")?;
        
        let mut parts = bind_addr.split(':');
        let local_ip: std::net::Ipv4Addr = parts.next().ok_or("missing bind IP")?.parse()?;
        parts.next().ok_or("missing bind port")?;
        
        // bind and join
        let socket = UdpSocket::bind(bind_addr).await?;
        socket.join_multicast_v4(dest_ip, local_ip)?;

        Ok(Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            socket: Arc::new(socket),
            peer_info,
            dest: dest_addr.parse()?,
        })
    }

    /// run the discovery service
    /// we pass self as an Arc because the uses itself to run the functions
    pub async fn run(self: Arc<Self>) {
        // clone out into locals so they live long enough
        let svc_listen = self.clone();
        let svc_announce = self.clone();
        let svc_sweep = self.clone();

        tokio::join!(
            svc_listen.listen_to_peers(),
            svc_announce.announce_presence(),
            svc_sweep.sweep_timeout_peers(),
        );
    }

    /// listen to incoming broadcast from the multicast address and store into peer map
    async fn listen_to_peers(&self) {
        let mut buf = [0u8; 1024];
        loop {
            // read from udp socket into mutable buffer of 1024 byte
            let (len, _src) = match self.socket.recv_from(&mut buf).await {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("Error reading from socket: {}", e);
                    continue;
                }
            };

            // deserialize bytes -> peer info wire
            let peer_info_wire = match bincode::deserialize::<PeerInfoWire>(&buf[..len]) {
                Ok(piw) => piw,
                Err(e) => {
                    eprintln!("Failed to deserialize bytes into PeerInfoWire: {}", e);
                    continue;
                }
            };

            // convert peer info wire to peer info
            let peer_info = match PeerInfo::try_from(peer_info_wire) {
                Ok(pi) => pi,
                Err(e) => {
                    eprintln!("Failed to parse PeerInfoWire to PeerInfo: {}", e);
                    continue;
                }
            };

            // once passed all, acquire lock and insert into map
            let mut peers_map = self.peers.lock().await;
            peers_map.insert(peer_info.peer_id, (peer_info, Instant::now()));
        }
    }

    /// broadcast current peer info to multicast address for other peers
    async fn announce_presence(&self) {
        let piw = PeerInfoWire::from(self.peer_info.clone());
        let data = bincode::serialize(&piw).unwrap();
        let mut interval = time::interval(ANNOUNCE_INTERVAL);

        // run intervals to broadcast one's peer info wire
        loop {
            interval.tick().await;
            // send peer info wire in bytes to multicast address
            if let Err(e) = self.socket.send_to(&data,self.dest).await {
                eprintln!("Broadcast error: {}", e);
            }
        }
    }

    /// Remove any stale peers *once*.
    pub async fn sweep_once(&self) {
        let mut peers_map = self.peers.lock().await;
        peers_map.retain(|_, (_, seen)| seen.elapsed() <= PEER_TIMEOUT);
    }

    /// Continuously run `sweep_once` every second.
    async fn sweep_timeout_peers(&self) {
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            self.sweep_once().await;
        }
    }

    /// retrieve the existing peers in the discovery service
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        let peers_map = self.peers.lock().await;
        peers_map
            .iter()
            .map(|(_peer_id, (peer_info, _instant))| peer_info.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::Ipv4Addr, sync::Arc, time::Duration};
    use tokio::time;

    #[tokio::test]
    /// testing for serializing and deserializing peer info wire
    async fn peer_info_wire_roundtrip() {
        let pi = test_peer_info();
        let wire: PeerInfoWire = pi.clone().into();
        let bytes = bincode::serialize(&wire).unwrap();
        let wire2: PeerInfoWire = bincode::deserialize(&bytes).unwrap();
        let pi2 = PeerInfo::try_from(wire2).unwrap();
        assert_eq!(pi.peer_id, pi2.peer_id);
        assert_eq!(pi.spare_mbs, pi2.spare_mbs);
        assert_eq!(pi.price, pi2.price);
    }

    // #[tokio::test]
    // /// discovery roundtrip loop back
    // async fn discovery_roundtrip_on_loopback() {
    //     let svc_a = Arc::new(DiscoveryService::with_addr(
    //         test_peer_info(),
    //         "127.0.0.1:6000",
    //         "127.0.0.1:6001",
    //     ).await.unwrap());
    //     let svc_b = Arc::new(DiscoveryService::with_addr(
    //         test_peer_info(),
    //         "127.0.0.1:6002", 
    //         "127.0.0.1:6001",
    //     ).await.unwrap());

    //     // run both services 
    //     tokio::spawn(svc_a.clone().run());
    //     tokio::spawn(svc_b.clone().run());

    //     // let them announce and listen
    //     time::sleep(Duration::from_secs(3)).await;

    //     // get peers
    //     let peers_a = svc_a.get_peers().await;
    //     let peers_b = svc_b.get_peers().await;

    //     // check
    //     assert!(
    //         peers_a.iter().any(|p| p.peer_id == svc_b.peer_info.peer_id),
    //         "A should see B"
    //     );
    //     assert!(
    //         peers_b.iter().any(|p| p.peer_id == svc_a.peer_info.peer_id),
    //         "B should see A"
    //     );
    // }

    #[tokio::test]
    /// sweep stale peer
    async fn sweep_stale_peer() {
        time::pause();
        let svc = Arc::new(DiscoveryService::new(test_peer_info()).await.unwrap());
        // run in block to drop reference and unlock the peers map
        {
            let mut map = svc.peers.lock().await;
            let pi = test_peer_info();
            map.insert(
                pi.peer_id,
                (
                    pi,
                    Instant::now() - Duration::from_secs(PEER_TIMEOUT.as_secs() + 1),
                ),
            );
        }
        svc.sweep_once().await;
        assert!(svc.get_peers().await.is_empty());
    }

    fn test_peer_info() -> PeerInfo {
        PeerInfo {
            peer_id: PeerId::random(),
            spare_mbs: 11,
            price: 11.0,
        }
    }
}
