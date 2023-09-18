use anyhow::*;
use clap::Clap;
use futures::StreamExt;
use quinn::{
    Certificate, CertificateChain, Connecting, Endpoint, NewConnection, PrivateKey, ServerConfig,
    ServerConfigBuilder, TransportConfig,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

#[derive(Clap, Debug)]
#[clap(version = "0.1.0")]
struct Opts {
    #[clap(short, long)]
    port: u16,
    #[clap(short, long)]
    ca: PathBuf,
    #[clap(long)]
    privkey: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // コマンドライン引数のパース
    let opts: Opts = Opts::parse();

    // QUICの設定
    let mut transport_config = TransportConfig::default();
    transport_config.stream_window_uni(0xFF);
    let mut server_config = ServerConfig::default();
    server_config.transport = std::sync::Arc::new(transport_config);
    let mut server_config = ServerConfigBuilder::new(server_config);
    // 証明書の設定
    let cert = Certificate::from_der(&std::fs::read(opts.ca)?)?;
    server_config.certificate(
        CertificateChain::from_certs(vec![cert]),
        PrivateKey::from_der(&std::fs::read(opts.privkey)?)?,
    )?;
    // QUICを開く
    let mut endpoint = Endpoint::builder();
    endpoint.listen(server_config.build());
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), opts.port);
    let (endpoint, mut incoming) = endpoint.bind(&addr)?;
    println!("listeing on {}", endpoint.local_addr()?);

    // クライアントからの接続を扱う
    while let Some(conn) = incoming.next().await {
        tokio::spawn(async {
            // クライアントとの処理を行い、エラーが起きたら表示
            match handle_connection(conn).await {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("{}", e);
                }
            }
        });
    }

    Ok(())
}

// echoの処理をする関数
async fn handle_connection(conn: Connecting) -> Result<(), Error> {
    let NewConnection {
        connection,
        mut uni_streams,
        ..
    } = conn.await?;

    println!("connected from {}", connection.remote_address());

    // 受信用のストリームを開く
    if let Some(uni_stream) = uni_streams.next().await {
        let uni_stream = uni_stream?;
        // ストリームを読み出す
        let data = uni_stream.read_to_end(0xFF).await?;
        println!("received \"{}\"", String::from_utf8_lossy(&data));
        // 送信用のストリームを開く
        let mut send_stream = connection.open_uni().await?;
        // 返信を書き込む
        send_stream.write(&data).await?;
        send_stream.finish().await?;
        connection.close(0u8.into(), &[]);
    } else {
        bail!("cannot open uni stream");
    }

    println!("closed");

    Ok(())
}
