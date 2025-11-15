use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::net::SocketAddr;

/// In-memory representation of a peer.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub peer_id: PeerId,
    pub spare_mbs: u64,
    pub price: f32,
}

/// Wire representation used for serialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfoWire {
    pub addr: SocketAddr,
    pub peer_id_bytes: ByteBuf,
    pub spare_mbs: u64,
    pub price: f32,
}

impl From<PeerInfo> for PeerInfoWire {
    fn from(pi: PeerInfo) -> Self {
        Self {
            addr: pi.addr,
            peer_id_bytes: ByteBuf::from(pi.peer_id.to_bytes()),
            spare_mbs: pi.spare_mbs,
            price: pi.price,
        }
    }
}

impl TryFrom<PeerInfoWire> for PeerInfo {
    type Error = libp2p::identity::ParseError;

    fn try_from(w: PeerInfoWire) -> Result<Self, Self::Error> {
        Ok(Self {
            addr: w.addr,
            peer_id: PeerId::from_bytes(&w.peer_id_bytes)?,
            spare_mbs: w.spare_mbs,
            price: w.price,
        })
    }
}
