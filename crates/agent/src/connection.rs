use anyhow::{Context, Error, Ok, Result};
use quinn::{Endpoint, ServerConfig};
use rustls::pki_types::PrivateKeyDer;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Serialize, Deserialize)]
pub struct Deal {
    pub file_len: u64,
    pub price_per_mb: f32,
}

///
///
///
pub async fn serve_custom_control(listen_addr: SocketAddr) -> Result<Deal> {
    let certified_key = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert = certified_key;
    let cert_der = cert.cert.der();
    let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der())
        .map_err(Error::msg)
        .context("failed to parse key into DER")?;
    let cert_chain = vec![cert_der.to_owned()];
    let svr_cfg = ServerConfig::with_single_cert(cert_chain, key_der)?;
    let ep = Endpoint::server(svr_cfg, listen_addr)?;

    // the ".context(<msg>)" will convert the error into anyhow::Error
    // and leave the Ok(T) untouched hence we need to "?" to retrieve T.
    let mut conn = ep
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

pub async fn send_deal(peer_addr: SocketAddr, deal: Deal) -> Result<()> {
    let mut client_cfg = quinn::ClientConfig::with_platform_verifier();
    let mut ep = Endpoint::client("0.0.0.0:0".parse().unwrap())
        .context("failed to create client endpoint")?;
    ep.set_default_client_config(client_cfg);

    // opening the connection
    let connect = ep
        .connect(peer_addr, "localhost")
        .context("failed to start connection")?;
    let connection = connect
        .await
        .context("connection handshake failed")?;

    let mut uni = connection
        .open_uni()
        .await
        .context("failed to open uni stream")?;

    let bytes = bincode::serialize(&deal).context("failed to serialize deal")?;
    uni.write_all(&bytes)
        .await
        .context("failed to write into uni stream")?;
    uni.finish().context("finishing stream")?;
    Ok(())
}
