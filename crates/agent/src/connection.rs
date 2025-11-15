use anyhow::{Context, Error, Result};
use quinn::{Endpoint, ServerConfig};
use rustls::{crypto::ring, pki_types::PrivateKeyDer};
use std::{net::SocketAddr, sync::Once};

use crate::deal::Deal;

#[cfg(test)]
use {libp2p::PeerId, quinn::crypto::rustls::QuicClientConfig, std::sync::Arc};

fn ensure_crypto_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        ring::default_provider()
            .install_default()
            .expect("failed to install ring provider")
    });
}

pub async fn open_receiver_endpoint(listen_addr: SocketAddr) -> Result<Endpoint> {
    ensure_crypto_provider();
    // QUIC requires TLS, so mint a throwaway self-signed certificate for this endpoint.
    let certified_key = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = certified_key.cert.der();
    let key_der = PrivateKeyDer::try_from(certified_key.key_pair.serialize_der())
        .map_err(Error::msg)
        .context("failed to parse key into DER")?;
    let cert_chain = vec![cert_der.to_owned()];
    let svr_cfg = ServerConfig::with_single_cert(cert_chain, key_der)?;
    Ok(Endpoint::server(svr_cfg, listen_addr)?)
}

/// Start a QUIC listener on `listen_addr`, read a single unidirectional stream,
/// and deserialize the payload into a [`Deal`].
pub async fn receive(endpoint: &Endpoint) -> Result<Deal> {
    // Each `accept()` unwraps one layer: endpoint -> incoming connections -> QUIC handshake.
    let conn = endpoint
        .accept()
        .await
        .context("no incoming")?
        .accept()
        .context("connection handshake failed")?
        .await?;
    let mut uni = conn
        .accept_uni()
        .await
        .context("failed to accept unidirectional stream")?;
    let bytes = uni
        .read_to_end(1024)
        .await
        .context("failed to read from unidirectional stream")?;
    let deal: Deal = bincode::deserialize(&bytes).context("deserializing deal")?;
    Ok(deal)
}

pub async fn open_sender_endpoint() -> Result<Endpoint> {
    ensure_crypto_provider();
    let client_cfg = default_client_config();
    let mut ep = Endpoint::client("0.0.0.0:0".parse().unwrap())
        .context("failed to create client endpoint")?;
    ep.set_default_client_config(client_cfg);
    Ok(ep)
}

/// Establish a QUIC connection to `peer_addr` and push a [`Deal`] over a
/// unidirectional stream.
pub async fn send(endpoint: &Endpoint, peer_addr: SocketAddr, deal: Deal) -> Result<()> {
    // Dial the remote endpoint; the hostname must match what the server's cert expects.
    let connect = endpoint
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

    // Closing the stream and connection
    uni.finish()?;
    connection.closed().await;
    Ok(())
}

#[cfg(not(test))]
fn default_client_config() -> quinn::ClientConfig {
    quinn::ClientConfig::with_platform_verifier()
}

#[cfg(test)]
fn default_client_config() -> quinn::ClientConfig {
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme};

    #[derive(Debug)]
    struct AcceptAnyCert;

    impl ServerCertVerifier for AcceptAnyCert {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, RustlsError> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::ECDSA_NISTP521_SHA512,
                SignatureScheme::ED25519,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PSS_SHA384,
                SignatureScheme::RSA_PSS_SHA512,
            ]
        }
    }

    let crypto = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth();

    let quic_crypto =
        QuicClientConfig::try_from(crypto).expect("failed to create QUIC client config");

    quinn::ClientConfig::new(Arc::new(quic_crypto))
}

#[cfg(test)]
mod tests {
    use serde_bytes::ByteBuf;

    use super::*;
    use crate::{deal::BYTES_PER_MEBIBYTE, peer_info::PeerInfoWire};

    #[tokio::test]
    #[ignore = "requires local QUIC handshake"]
    async fn round_trip_control_deal() {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap_or_else(|err| {
            eprintln!("failed to parse into socket {}", err);
            panic!("failed to parse into socket address");
        });

        let rep = open_receiver_endpoint(addr).await.unwrap_or_else(|err| {
            eprintln!("failed to open receiving quic endpoint {}", err);
            panic!("failed to open receiver quic endpoint");
        });

        let sep = open_sender_endpoint().await.unwrap_or_else(|err| {
            eprintln!("failed to open sending quic endpoint {}", err);
            panic!("failed to open sender quic endpoint");
        });

        let ep_thread = rep.clone();
        let server = tokio::spawn(async move { receive(&ep_thread).await });

        let deal = Deal {
            peer_info_wire: PeerInfoWire {
                addr: addr,
                peer_id_bytes: ByteBuf::from(PeerId::random().to_bytes()),
                spare_mbs: 10,
                price: 10.0,
            },
            file_len: 10 * BYTES_PER_MEBIBYTE,
            price_per_mb: 10.0,
        };

        match send(&sep, addr, deal.clone()).await {
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
