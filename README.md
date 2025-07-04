# spare-net

A lightweight, peer-to-peer resource-sharing tool that negotiates transfer "Deals" over an encrypted QUIC/TLS control plane and discovers peers via mDNS.

## Progress to Date

1. **Project Setup**

   * Initialized Rust workspace with `Tokio`, `quinn`, `rcgen`, `bincode`, and mDNS crates.

2. **Peer Discovery**

   * Implemented mDNS-based discovery service:

     * Periodic announcements of node presence.
     * Background listener to decode incoming `PeerInfo` via `bincode`.
     * Stale-peer pruning task to expire inactive entries.

3. **Control Plane**

   * Defined a `Deal { file_len, price_per_mb }` struct (Serde + `bincode`).
   * Built `serve_custom_control`: generates self-signed TLS cert, spins up a QUIC server, accepts one connection, reads a `Deal`.
   * Built `send_deal`: configures a QUIC client, dials a peer, opens a unidirectional stream, sends a serialized `Deal` with error contexts.

4. **CLI Integration & Testing**

   * Exposed commands:

     * `sparenet peers` — snapshot current peer map.
     * `sparenet get <peer>` — negotiate the `Deal` (client side).
     * `sparenet lend <file>` — wait for incoming `Deal` (server side).
   * Authored loopback integration tests to validate end-to-end `Deal` round trips.

## Pending

* **Data Plane Implementation**

  * Chunking and scheduling of resource transfers (files, proxy streams).
  * Parallel streams, resume support, and integrity checks.

* **Pricing & Quota Enforcement**

  * Track usage (bytes transferred) against `price_per_mb`.
  * On-chain or off-chain credit accounting.

* **User Experience Enhancements**

  * CLI flags for pricing negotiation.
  * Progress bars and transfer statistics.

