use anyhow::{anyhow, Result};
use quinn::{Endpoint, ServerConfig};
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

pub struct QuicServer {
    pub endpoint: Endpoint,
    local_addr: SocketAddr,
}

impl QuicServer {
    pub async fn new(addr: SocketAddr) -> Result<Self> {
        let (cert, key) = generate_self_signed_cert()?;
        let server_config = configure_server(cert, key)?;
        
        let endpoint = Endpoint::server(server_config, addr)?;
        let local_addr = endpoint.local_addr()?;
        
        info!("QUIC server listening on {}", local_addr);
        
        Ok(Self {
            endpoint,
            local_addr,
        })
    }
    
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    
    pub async fn run(&self) -> Result<()> {
        info!("Server is ready to accept connections");
        
        while let Some(incoming) = self.endpoint.accept().await {
            let connection = incoming.await?;
            let remote_addr = connection.remote_address();
            info!("Connection accepted from: {}", remote_addr);
            
            tokio::spawn(async move {
                if let Err(e) = handle_connection(connection).await {
                    error!("Connection error: {}", e);
                }
            });
        }
        
        Ok(())
    }
}

async fn handle_connection(connection: quinn::Connection) -> Result<()> {
    info!("Handling connection from: {}", connection.remote_address());
    
    loop {
        match connection.accept_bi().await {
            Ok((mut send, mut recv)) => {
                info!("Accepted bidirectional stream");
                
                let buffer = recv.read_to_end(64 * 1024).await?;
                
                let message = String::from_utf8(buffer)?;
                info!("Received message: {}", message);
                
                let response = format!("Echo: {}", message);
                send.write_all(response.as_bytes()).await?;
                send.finish()?;
                
                info!("Sent response: {}", response);
            }
            Err(quinn::ConnectionError::ApplicationClosed(_)) => {
                info!("Connection closed by peer");
                break;
            }
            Err(e) => {
                error!("Connection error: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

fn generate_self_signed_cert() -> Result<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>)> {
    let cert = generate_simple_self_signed(vec!["localhost".to_string()])?;
    let cert_der = cert.cert.der().clone();
    let key_der = cert.key_pair.serialize_der();
    
    Ok((cert_der, key_der.try_into()?))
}

fn configure_server(
    cert: CertificateDer<'static>,
    key: PrivatePkcs8KeyDer<'static>,
) -> Result<ServerConfig> {
    let mut crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key.into())?;
    
    crypto.alpn_protocols = vec![b"quic-echo".to_vec()];
    
    let server_config = ServerConfig::with_crypto(Arc::new(quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?));
    Ok(server_config)
}