# Sparenet

Sparenet is a peer-to-peer experimentation space for sharing spare network
capacity. Each node advertises how many megabytes it can serve and at what
price, discovers other nodes on the local network, and negotiates transfer
“deals” over a QUIC control channel.

```
                ┌───────────────────────┐
                │   UDP Discovery Bus   │  multicast/loopback socket
                └─────────┬─────────────┘
                          │
                   ┌──────▼──────┐
                   │ Discovery   │ announces PeerInfo,
          ┌────────┤ Service     │ listens/prunes peers
          │        └──────-┬─────┘
          │                │
┌─────────▼───────┐   ┌────▼────────---┐
│ Agent (node A)  │   │ Agent (node B) │
│ - discovery     │   │ - discovery    │
│ - sender QUIC   │◄─ ┤ - receiver QUIC│
│ - receiver QUIC │──►│ - sender QUIC  │
└─────────────────┘   └──────────────--┘
```

Nodes keep one UDP socket for discovery (bidirectional multicast) and two QUIC
endpoints: a listener bound to the advertised control port and a reusable client
endpoint for dialing peers. Deals embed the sender’s `PeerInfoWire`, so the
receiver learns who offered the contract even though QUIC only exposes the
ephemeral source socket.

## Repository Layout

- `crates/agent`
  - `discovery`: UDP multicast (or loopback) presence broadcasting plus a peer
    map that expires entries after inactivity.
  - `connection`: QUIC-based control plane for proposing and accepting `Deal`s
    (now carrying the sender’s `PeerInfoWire`, file length, and price).
  - `agent`: Ties discovery and QUIC together, reusing endpoints, matching deals,
    logging via `tracing`, and storing incoming deals keyed by sender addr.
- `crates/cli`: Placeholder CLI where the eventual user experience for running
  an agent, listing peers, and initiating deals will live.

## How Things Fit Together

1. `DiscoveryService::start` spawns three async tasks:
   - `announce_presence`: serializes `PeerInfoWire`, prefixes with `MAGIC_HEADER`
     and sends over the shared UDP socket every `ANNOUNCE_INTERVAL`.
   - `listen_to_peers`: reads from the same socket, filters by header, decodes
     `PeerInfoWire`, and updates an `Arc<Mutex<HashMap<PeerId, PeerInfo>>>`.
   - `sweep_timeout_peers`: prunes entries that exceed `PEER_TIMEOUT`.
2. `Agent::run` holds `Arc<DiscoveryService>`, a QUIC receiver endpoint bound to
   its advertised `PeerInfo.addr`, a shared QUIC client endpoint for sending,
   and an `incoming_deals` map. It spawns discovery and a `receive_deals` loop.
3. `send_matched_deals` clones the sender endpoint, filters peers via
   `deal_match`, and dispatches `connection::send`, which opens a unidirectional
   QUIC stream, writes the serialized `Deal`, and finishes the stream.
4. `receive_deals` awaits `connection::receive`, logs the sender address from
   the deal’s `PeerInfoWire`, and records the contract.

## Developing

```bash
# Run all agent tests (discovery, connection, agent)
cargo test -p sparenet-agent

# Work on the CLI once commands exist
cargo run -p sparenet-cli -- --help
```

Tracing is available via `RUST_LOG=sparenet_agent=info,quinn=warn`.

## Near-Term Direction

1. Hook up the CLI so running `sparenet-cli` spins up discovery and shows live
   peers/deals.
2. Extend the QUIC round-trip test (currently `#[ignore]`) and add an integration
   test that exercises discovery plus a deal exchange.
3. Define the data transfer/payment workflow that follows a `Deal`, including
   authenticated TLS (CA-backed certificates or signed deals) and actual data
   movement once a contract is accepted.
