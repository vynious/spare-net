use libp2p::{futures::lock::Mutex, PeerId};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{collections::HashMap, error::Error, f128::consts::E, sync::Arc, time::Instant};
use tokio::net::UdpSocket;

// in-memory representation
#[derive(Debug)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub spare_mbs: u16,
    pub price: f32,
}

// wire representation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfoWire {
    pub peer_id_bytes: ByteBuf,
    pub spare_mbs: u16,
    pub price: f32,
}

// convert PeerInfo into PeerInfoWire (in-memory -> wire)
impl From<PeerInfo> for PeerInfoWire {
    fn from(pi: PeerInfo) -> Self {
        PeerInfoWire {
            peer_id_bytes: ByteBuf::from(pi.peer_id.to_bytes()),
            spare_mbs: pi.spare_mbs,
            price: pi.price,
        }
    }
}

// convert PeerInfoWire into PeerInfo (wire -> in-memory)
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
}

impl DiscoveryService {
    // creates a new discovery service based on the peer_info
    pub async fn new(peer_info: PeerInfo) -> Result<Self, Box<dyn Error>> {
        let multicast_ip = "224.0.0.251".parse()?;
        let local_ip = "0.0.0.0".parse()?;
        let socket = UdpSocket::bind("0.0.0.0:5333").await?;
        socket.join_multicast_v4(multicast_ip, local_ip)?;
        Ok(Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            socket: Arc::new(socket),
            peer_info: peer_info,
        })
    }

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

    async fn send_to_peers(&self) {

    }
}
