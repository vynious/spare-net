use serde::{Deserialize, Serialize};

use crate::peer_info::PeerInfoWire;

/// Number of bytes in one mebibyte (MiB).
pub const BYTES_PER_MEBIBYTE: u64 = 1024 * 1024;

/// Describes a storage deal request between peers.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Deal {
    pub peer_info_wire: PeerInfoWire,
    /// File length in bytes.
    pub file_len: u64,
    pub price_per_mb: f32,
}
