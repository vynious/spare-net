use libp2p::PeerId;
use serde::{Serialize, Deserialize};
use serde_bytes::ByteBuf;


// in-memory representation
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
        PeerInfoWire { peer_id_bytes: ByteBuf::from(pi.peer_id.to_bytes()), spare_mbs: pi.spare_mbs, price: pi.price }
    }
}

// convert PeerInfoWire into PeerInfo (wire -> in-memory)
impl TryFrom<PeerInfoWire> for PeerInfo {
    type Error = libp2p::identity::ParseError;
    fn try_from(w: PeerInfoWire) -> Result<Self, Self::Error> {
        Ok(PeerInfo { peer_id: PeerId::from_bytes(&w.peer_id_bytes)?, spare_mbs: w.spare_mbs, price: w.price })
    }
}