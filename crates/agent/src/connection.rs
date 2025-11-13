use anyhow::{Context, Error, Result};
use quinn::{Endpoint, ServerConfig};
use rustls::{crypto::ring, pki_types::PrivateKeyDer};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Once};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Deal {
    pub file_len: u64,
    pub price_per_mb: f32,
}

fn ensure_crypto_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        ring::default_provider()
            .install_default()
            .expect(("failed to install ring provider"))
    });
}

/// Start a QUIC listener on `listen_addr`, read a single unidirectional stream,
/// and deserialize the payload into a [`Deal`].
pub async fn receive_connection(listen_addr: SocketAddr) -> Result<Deal> {
    ensure_crypto_provider();
    // QUIC requires TLS, so mint a throwaway self-signed certificate for this endpoint.
    let certified_key = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = certified_key.cert.der();
    let key_der = PrivateKeyDer::try_from(certified_key.key_pair.serialize_der())
        .map_err(Error::msg)
        .context("failed to parse key into DER")?;
    let cert_chain = vec![cert_der.to_owned()];
    let svr_cfg = ServerConfig::with_single_cert(cert_chain, key_der)?;
    let ep = Endpoint::server(svr_cfg, listen_addr)?;

    // Each `accept()` unwraps one layer: endpoint -> incoming connections -> QUIC handshake.
    let conn = ep
        .accept()
        .await
        .context("no incoming")?
        .accept()
        .context("connection handshake failed")?
        .await?;
    let mut uni = conn
        .accept_uni()
        .await
        .context("fialed to accept uni stream")?;
    let bytes = uni
        .read_to_end(1024)
        .await
        .context("reading deal bytes from stream")?;
    let deal: Deal = bincode::deserialize(&bytes).context("deserializing deal")?;
    Ok(deal)
}

/// Establish a QUIC connection to `peer_addr` and push a [`Deal`] over a
/// unidirectional stream.
pub async fn send_deal(peer_addr: SocketAddr, deal: Deal) -> Result<()> {
    ensure_crypto_provider();
    let client_cfg = quinn::ClientConfig::with_platform_verifier();
    let mut ep = Endpoint::client("0.0.0.0:0".parse().unwrap())
        .context("failed to create client endpoint")?;
    ep.set_default_client_config(client_cfg);

    // Dial the remote endpoint; the hostname must match what the server's cert expects.
    let connect = ep
        .connect(peer_addr, "localhost")
        .context("failed to start connection")?;
    let connection = connect.await.context("connection handshake failed")?;

    // Open a unidirectional stream for the control payload.
    let mut uni = connection
        .open_uni()
        .await
        .context("failed to open uni stream")?;

    // Serialize and transmit the deal, then gracefully finish the stream.
    let bytes = bincode::serialize(&deal).context("failed to serialize deal")?;
    uni.write_all(&bytes)
        .await
        .context("failed to write into uni stream")?;
    uni.finish().context("finishing stream")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires local QUIC handshake"]
    async fn round_trip_control_deal() {
        let deal = Deal {
            file_len: 10,
            price_per_mb: 10.0,
        };

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap_or_else(|err| {
            eprintln!("failed to parse into socket {}", err);
            panic!("failed to parse into socket address");
        });

        let server = tokio::spawn(receive_connection(addr));

        match send_deal(addr, deal.clone()).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error sending deal: {}", e)
            }
        };

        let received_deal = server
            .await
            .unwrap_or_else(|err| {
                eprintln!("join error: {err}");
                panic!("task panic");
            })
            .unwrap_or_else(|err| {
                eprintln!("receive_connection error: {err}");
                panic!("failed to receive connection");
            });

        assert_eq!(received_deal.file_len, deal.file_len);
        assert_eq!(received_deal.price_per_mb, deal.price_per_mb);
    }
}
