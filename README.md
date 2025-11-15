# Sparenet

Sparenet is a peer-to-peer experimentation space for sharing spare network
capacity. Each node advertises how many megabytes it can serve and at what
price, discovers other nodes on the local network, and negotiates transfer
“deals” over a QUIC control channel.

```
                ┌───────────────────────┐
                │   UDP Discovery Bus   │  multicast/loopback socket
                └───────────────────────┘

┌──────────────────────────────┐           ┌──────────────────────────────┐
│ Agent (node A)               │◄────────► │ Agent (node B)               │
│ ┌──────────────────────────┐ │           │ ┌──────────────────────────┐ │
│ │ Discovery Service        │◄┼───UDP─────┼►│ Discovery Service        │ │
│ │ (announces/prunes peers) │ │           │ │ (announces/prunes peers) │ │
│ └──────────────────────────┘ │           │ └──────────────────────────┘ │
│ sender QUIC ◄──────────────┐ │           │ sender QUIC ◄──────────────┐ │
│ receiver QUIC ────────────►│ │           │ receiver QUIC ────────────►│ │
└──────────────────────────────┘           └──────────────────────────────┘
```

Nodes keep one UDP socket for discovery (bidirectional multicast) and two QUIC
endpoints: a listener bound to the advertised control port and a reusable client
endpoint for dialing peers. Deals embed the sender’s `PeerInfoWire`, so the
receiver learns who offered the contract even though QUIC only exposes the
ephemeral source socket.


## Developing

```bash
# Run all agent tests (discovery, connection, agent)
cargo test -p sparenet-agent

# Work on the CLI once commands exist
cargo run -p sparenet-cli -- --help
```
