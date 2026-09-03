//! Native Arena client.
//!
//!     arena-app                                              # Arena v0, local 2P
//!     arena-app online URL create|join LOBBY [PASSWORD|-] [HANDLE] [MAP]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("online") {
        let usage = "usage: arena-app online URL create|join LOBBY [PASSWORD|-] [HANDLE] [MAP]";
        let cfg = arena::OnlineConfig {
            url: args.get(1).expect(usage).clone(),
            action: args.get(2).expect(usage).clone(),
            lobby: args.get(3).expect(usage).clone(),
            password: args.get(4).filter(|p| !p.is_empty() && *p != "-").cloned(),
            handle: args
                .get(5)
                .cloned()
                .or_else(|| std::env::var("USERNAME").ok())
                .unwrap_or_else(|| "player".into()),
            // Only a `create` reads it; empty lets the server pick its default map.
            map: args.get(6).cloned().unwrap_or_default(),
        };
        if let Err(e) = arena::run_online(cfg) {
            tracing::error!(error = %e, "online mode failed");
            return ExitCode::FAILURE;
        }
    } else {
        arena::run_local();
    }
    ExitCode::SUCCESS
}
