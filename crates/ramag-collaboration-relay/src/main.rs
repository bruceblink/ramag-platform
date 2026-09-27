use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let address = std::env::var("RAMAG_RELAY_BIND")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 18080));
    ramag_collaboration_relay::run(address).await
}
