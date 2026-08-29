//! Client-side connection: a reader thread feeds ServerMsgs into a channel
//! the game loop drains once per frame. Writes are mutex-guarded because two
//! threads produce them: the game loop (inputs) and a keepalive thread that
//! keeps the server timeout at bay even when the window is minimized and the
//! render loop stalls.

use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ember_net::{
    read_msg, write_msg, ClientMsg, PlayerId, PlayerMeta, ServerMsg, PROTOCOL_VERSION,
};

/// The server snapshots at 60 Hz; this much silence means it is gone.
const SERVER_SILENCE_TIMEOUT: Duration = Duration::from_secs(15);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(2);

pub struct Welcome {
    pub id: PlayerId,
    pub tick_hz: u32,
    pub arena_half: f32,
    pub roster: Vec<PlayerMeta>,
}

pub struct NetClient {
    stream: Arc<Mutex<TcpStream>>,
    pub rx: Receiver<ServerMsg>,
    dead: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    /// Epoch for Ping nonces: pings carry `started.elapsed()` in ms, so a
    /// Pong's nonce subtracted from the current elapsed time is the RTT.
    started: std::time::Instant,
}

impl NetClient {
    pub fn connect(addr: &str, name: &str) -> io::Result<(NetClient, Welcome)> {
        let mut last_err = None;
        let mut stream = None;
        for sock_addr in addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&sock_addr, Duration::from_secs(4)) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        let mut stream = stream.ok_or_else(|| {
            last_err.unwrap_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no address"))
        })?;
        stream.set_nodelay(true)?;

        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        write_msg(
            &mut stream,
            &ClientMsg::Hello {
                protocol: PROTOCOL_VERSION,
                name: name.to_string(),
            },
        )?;
        let welcome = match read_msg::<_, ServerMsg>(&mut stream)? {
            ServerMsg::Welcome {
                id,
                tick_hz,
                arena_half,
                roster,
            } => Welcome {
                id,
                tick_hz,
                arena_half,
                roster,
            },
            ServerMsg::Reject { reason } => {
                return Err(io::Error::new(io::ErrorKind::ConnectionRefused, reason));
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("expected Welcome, got {other:?}"),
                ));
            }
        };
        // Steady state: any read error — including this timeout, i.e. total
        // server silence — means the connection is dead.
        stream.set_read_timeout(Some(SERVER_SILENCE_TIMEOUT))?;

        let (tx, rx) = mpsc::channel();
        let dead = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut reader = stream.try_clone()?;
            let dead = Arc::clone(&dead);
            std::thread::spawn(move || {
                loop {
                    match read_msg::<_, ServerMsg>(&mut reader) {
                        Ok(msg) => {
                            if tx.send(msg).is_err() {
                                break; // game gone
                            }
                        }
                        Err(_) => break, // server gone (or silent too long)
                    }
                }
                dead.store(true, Ordering::Relaxed);
            });
        }

        let started = std::time::Instant::now();
        let stream = Arc::new(Mutex::new(stream));
        {
            let stream = Arc::clone(&stream);
            let stop = Arc::clone(&stop);
            let dead = Arc::clone(&dead);
            std::thread::spawn(move || {
                let step = Duration::from_millis(250);
                let mut since_ping = Duration::ZERO;
                loop {
                    std::thread::sleep(step);
                    if stop.load(Ordering::Relaxed) || dead.load(Ordering::Relaxed) {
                        break;
                    }
                    since_ping += step;
                    if since_ping >= KEEPALIVE_INTERVAL {
                        since_ping = Duration::ZERO;
                        // Timestamped nonce -> the Pong measures RTT.
                        let nonce = started.elapsed().as_millis() as u32;
                        let mut s = stream.lock().unwrap();
                        if write_msg(&mut *s, &ClientMsg::Ping { nonce }).is_err() {
                            break;
                        }
                    }
                }
            });
        }

        Ok((
            NetClient {
                stream,
                rx,
                dead,
                stop,
                started,
            },
            welcome,
        ))
    }

    /// Milliseconds since this connection's Ping epoch.
    pub fn elapsed_ms(&self) -> u32 {
        self.started.elapsed().as_millis() as u32
    }

    pub fn send(&mut self, msg: &ClientMsg) -> io::Result<()> {
        let mut s = self.stream.lock().unwrap();
        write_msg(&mut *s, msg)
    }

    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::Relaxed)
    }
}

impl Drop for NetClient {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut s) = self.stream.lock() {
            let _ = write_msg(&mut *s, &ClientMsg::Bye);
        }
    }
}
