use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub max_concurrent_streams: u32,
    pub keep_alive_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub bind_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub server_name: String,
    pub keep_alive_interval_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:4433".parse().unwrap(),
            max_concurrent_streams: 100,
            keep_alive_interval_secs: 5,
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            server_addr: "127.0.0.1:4433".parse().unwrap(),
            server_name: "localhost".to_string(),
            keep_alive_interval_secs: 5,
        }
    }
}