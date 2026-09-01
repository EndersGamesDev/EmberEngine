//! `fire-server [bind-addr]` — defaults to 127.0.0.1:7781.
//!
//! TLS terminates at the tunnel in front of this, exactly as it does for
//! arena-server, so the listener here is plain TCP.

use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let bind = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7781".to_string());
    let listener = TcpListener::bind(&bind)?;
    fire_server::run(listener, fire_server::ServerConfig::default())
}
