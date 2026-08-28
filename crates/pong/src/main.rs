//! Native pong.
//!
//!     pong-app                                              # local 2P
//!     pong-app online URL create|join LOBBY [PASSWORD|-] [HANDLE]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("online") {
        let usage = "usage: pong-app online URL create|join LOBBY [PASSWORD|-] [HANDLE]";
        let cfg = pong::OnlineConfig {
            url: args.get(1).expect(usage).clone(),
            action: args.get(2).expect(usage).clone(),
            lobby: args.get(3).expect(usage).clone(),
            password: args.get(4).filter(|p| !p.is_empty() && *p != "-").cloned(),
            handle: args
                .get(5)
                .cloned()
                .or_else(|| std::env::var("USERNAME").ok())
                .unwrap_or_else(|| "player".into()),
        };
        if let Err(e) = pong::run_online(cfg) {
            eprintln!("online mode failed: {e}");
            std::process::exit(1);
        }
    } else {
        pong::run_local();
    }
}
