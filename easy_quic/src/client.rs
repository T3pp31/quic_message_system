use anyhow::Result;
use quinn::{ClientConfig, Endpoint};
use rustls::pki_types::ServerName;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::info;

pub struct QuicClient {
    endpoint: Endpoint,
}

impl QuicClient {
    pub async fn new(bind_addr: SocketAddr) -> Result<Self> {
        let client_config = configure_client();
        let mut endpoint = Endpoint::client(bind_addr)?;
        endpoint.set_default_client_config(client_config);
        
        info!("QUIC client initialized on {}", endpoint.local_addr()?);
        
        Ok(Self { endpoint })
    }
    
    pub async fn connect(&self, server_addr: SocketAddr, server_name: &str) -> Result<ClientConnection> {
        info!("Connecting to {} ({})", server_addr, server_name);
        
        let connection = self.endpoint
            .connect(server_addr, server_name)?
            .await?;
            
        info!("Connected to server: {}", connection.remote_address());
        
        Ok(ClientConnection { connection })
    }
}

pub struct ClientConnection {
    connection: quinn::Connection,
}

impl ClientConnection {
    pub async fn send_message(&self, message: &str) -> Result<String> {
        info!("Sending message: {}", message);
        
        let (mut send, mut recv) = self.connection.open_bi().await?;
        
        send.write_all(message.as_bytes()).await?;
        send.finish()?;
        
        let response = recv.read_to_end(64 * 1024).await?;
        
        let response_str = String::from_utf8(response)?;
        info!("Received response: {}", response_str);
        
        Ok(response_str)
    }
    
    pub async fn close(&self) {
        self.connection.close(0u32.into(), b"done");
        info!("Connection closed");
    }
    
    pub fn remote_address(&self) -> SocketAddr {
        self.connection.remote_address()
    }
}

fn configure_client() -> ClientConfig {
    let crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();
    
    ClientConfig::with_crypto(Arc::new(quinn::crypto::rustls::QuicClientConfig::try_from(crypto).unwrap()))
}

#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}