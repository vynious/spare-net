# Sparenet

Peer-to-peer experimentation space for sharing spare network capacity. Nodes
advertise how many megabytes they can serve and at what price, discover each
other on the local network, and negotiate transfer “deals” over a QUIC control
channel.

## Repository Layout

- `crates/agent`: Core networking logic. It exposes:
  - `discovery`: UDP multicast (or loopback) presence broadcasting plus a peer
    map that expires entries after inactivity.
  - `connection`: QUIC-based control plane for proposing and accepting `Deal`s
    that describe file size and price per MB.
- `crates/cli`: Placeholder CLI where the eventual user interface for running an
  agent, listing peers, and initiating deals will live.

## How Things Fit Together

1. Each node instantiates `DiscoveryService`, which
   - broadcasts its `PeerInfo` (peer ID, spare MBs, asking price) every two
     seconds,
   - listens for other broadcasts and tracks live peers,
   - prunes any peer that has been silent longer than `PEER_TIMEOUT`.
2. When two peers decide to transact, they use `connection::send_deal` /
   `serve_custom_control` to establish a QUIC connection, authenticate with a
   self-signed certificate, and ship a serialized `Deal`.
3. The CLI will orchestrate these pieces so an operator can run a node, observe
   discovered peers, and exchange deals without writing custom code.

## Developing

```bash
# Run all agent tests (discovery and connection units)
cargo test -p sparenet-agent

# Work on the CLI once commands are added
cargo run -p sparenet-cli -- --help
```

## Near-Term Direction

1. Wire the CLI to the agent crate so running `sparenet-cli` spins up discovery
   and shows live peers.
2. Finish the QUIC round-trip test in `connection.rs` and add an integration
   test that covers discovery plus a deal exchange.
3. Define the actual data transfer/payment workflow that follows a `Deal`,
   extending the protocol as needed.
