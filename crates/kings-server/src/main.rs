//! `kings-server [bind] [--name <host name>]`; the bind defaults to
//! 127.0.0.1:7782 (7780 is the arena, 7781 the racer).
//!
//! TLS terminates at the tunnel in front of this, exactly as it does for the
//! other two servers, so the listener here is plain TCP. The host name goes
//! out in every `Welcome` (`docs/hosts.md`, section 4): `--name` first, else
//! the `EMBER_HOST_NAME` environment variable, else empty.

use std::io::IsTerminal;
use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    // The deploy appends stdout to a log file and later prints its first
    // line; colour codes there are noise, so they are only used on a tty.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let mut bind = "127.0.0.1:7782".to_string();
    let mut name: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--name" {
            name = args.next();
        } else {
            bind = arg;
        }
    }
    let host = name
        .or_else(|| std::env::var("EMBER_HOST_NAME").ok())
        .unwrap_or_default();

    let listener = TcpListener::bind(&bind)?;
    kings_server::run(
        listener,
        kings_server::ServerConfig {
            host,
            ..kings_server::ServerConfig::default()
        },
    )
}
