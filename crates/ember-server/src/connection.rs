//! WebSocket handshake and per-connection I/O ownership.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::net::{IpAddr, Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ember_legacy::{GameKey, MonotonicTimestamp};
use tungstenite::Message;
use tungstenite::handshake::server::{Request, Response};
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::{CloseFrame, WebSocketConfig};

use crate::capabilities::HostEpoch;

const READ_POLL: Duration = Duration::from_millis(5);

#[derive(Clone)]
pub(crate) struct ConnectionConfig {
    pub(crate) max_connections: usize,
    pub(crate) max_connections_per_ip: usize,
    pub(crate) cap_loopback: bool,
    pub(crate) outbound_queue_messages: usize,
    pub(crate) max_ws_message_bytes: usize,
    pub(crate) handshake_timeout: Duration,
    pub(crate) write_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Ingress {
    Canonical,
    Legacy(GameKey),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DataFrame {
    Text(String),
    Binary(Vec<u8>),
}

impl DataFrame {
    pub(crate) const fn byte_len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Binary(bytes) => bytes.len(),
        }
    }
}

pub(crate) enum ConnectionEvent {
    Connected {
        id: u64,
        outbound: SyncSender<OutboundCommand>,
        peer: String,
        ingress: Ingress,
    },
    Data {
        id: u64,
        frame: DataFrame,
        received_at: MonotonicTimestamp,
    },
    Control {
        id: u64,
    },
    OutboundDrained {
        id: u64,
        bytes: usize,
        version_frame: bool,
    },
    Disconnected {
        id: u64,
    },
}

pub(crate) enum OutboundCommand {
    Data {
        message: Message,
        bytes: usize,
        version_frame: bool,
    },
    Close {
        code: CloseCode,
        reason: String,
    },
}

pub(crate) fn spawn_acceptor(
    listener: TcpListener,
    events: SyncSender<ConnectionEvent>,
    config: ConnectionConfig,
    legacy_selectors: BTreeMap<String, GameKey>,
    epoch: HostEpoch,
) -> io::Result<()> {
    let legacy_selectors = Arc::new(legacy_selectors);
    let acceptor = thread::Builder::new()
        .name("ember-host-accept".to_string())
        .spawn(move || {
            let live_connections = Arc::new(AtomicUsize::new(0));
            let per_ip = Arc::new(Mutex::new(HashMap::<IpAddr, usize>::new()));
            let mut next_id = 1_u64;
            for accepted in listener.incoming() {
                let stream = match accepted {
                    Ok(stream) => stream,
                    Err(error) => {
                        tracing::warn!(%error, "listener accept failed; backing off");
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                };
                let ip = stream.peer_addr().ok().map(|address| address.ip());
                if live_connections.load(Ordering::Acquire) >= config.max_connections {
                    tracing::warn!("global connection admission cap reached");
                    drop(stream.shutdown(Shutdown::Both));
                    continue;
                }
                if let Some(address) = ip
                    && connection_ip_is_full(address, &per_ip, &config)
                {
                    tracing::warn!(?ip, "per-IP connection admission cap reached");
                    drop(stream.shutdown(Shutdown::Both));
                    continue;
                }

                let id = next_id;
                next_id = next_id.saturating_add(1);
                live_connections.fetch_add(1, Ordering::AcqRel);
                increment_ip(ip, &per_ip, &config);
                let event_sender = events.clone();
                let thread_config = config.clone();
                let thread_selectors = Arc::clone(&legacy_selectors);
                let thread_live_connections = Arc::clone(&live_connections);
                let thread_per_ip = Arc::clone(&per_ip);
                let connection = thread::Builder::new()
                    .name(format!("ember-host-connection-{id}"))
                    .spawn(move || {
                        run_connection(
                            id,
                            stream,
                            &event_sender,
                            &thread_config,
                            &thread_selectors,
                            epoch,
                        );
                        thread_live_connections.fetch_sub(1, Ordering::AcqRel);
                        decrement_ip(ip, &thread_per_ip, &thread_config);
                    });
                if let Err(error) = connection {
                    live_connections.fetch_sub(1, Ordering::AcqRel);
                    decrement_ip(ip, &per_ip, &config);
                    tracing::warn!(connection = id, %error, "connection thread could not start");
                }
            }
        })?;
    drop(acceptor);
    Ok(())
}

fn connection_ip_is_full(
    ip: IpAddr,
    per_ip: &Mutex<HashMap<IpAddr, usize>>,
    config: &ConnectionConfig,
) -> bool {
    if ip.is_loopback() && !config.cap_loopback {
        return false;
    }
    match per_ip.lock() {
        Ok(counts) => counts.get(&ip).copied().unwrap_or(0) >= config.max_connections_per_ip,
        Err(_) => true,
    }
}

fn increment_ip(
    ip: Option<IpAddr>,
    per_ip: &Mutex<HashMap<IpAddr, usize>>,
    config: &ConnectionConfig,
) {
    let Some(ip) = ip else {
        return;
    };
    if ip.is_loopback() && !config.cap_loopback {
        return;
    }
    if let Ok(mut counts) = per_ip.lock() {
        let count = counts.entry(ip).or_insert(0);
        *count = count.saturating_add(1);
    }
}

fn decrement_ip(
    ip: Option<IpAddr>,
    per_ip: &Mutex<HashMap<IpAddr, usize>>,
    config: &ConnectionConfig,
) {
    let Some(ip) = ip else {
        return;
    };
    if ip.is_loopback() && !config.cap_loopback {
        return;
    }
    if let Ok(mut counts) = per_ip.lock()
        && let Some(count) = counts.get_mut(&ip)
    {
        *count = count.saturating_sub(1);
        if *count == 0 {
            counts.remove(&ip);
        }
    }
}

// Keeping handshake, reads, and bounded writes together makes socket ownership auditable.
#[allow(clippy::too_many_lines)]
fn run_connection(
    id: u64,
    stream: TcpStream,
    events: &SyncSender<ConnectionEvent>,
    config: &ConnectionConfig,
    legacy_selectors: &BTreeMap<String, GameKey>,
    epoch: HostEpoch,
) {
    let peer = stream
        .peer_addr()
        .map_or_else(|_| "unknown".to_string(), |address| address.to_string());
    drop(stream.set_nodelay(true));
    drop(stream.set_read_timeout(Some(config.handshake_timeout)));
    drop(stream.set_write_timeout(Some(config.write_timeout)));

    let handshake_complete = Arc::new(AtomicBool::new(false));
    spawn_handshake_watchdog(
        &stream,
        config.handshake_timeout,
        Arc::clone(&handshake_complete),
    );
    let (ingress_sender, ingress_receiver) = mpsc::sync_channel(1);
    let selectors = legacy_selectors.clone();
    let callback = move |request: &Request, response: Response| {
        let ingress = parse_ingress(request.uri().query(), &selectors);
        drop(ingress_sender.try_send(ingress));
        Ok(response)
    };
    let websocket_config = WebSocketConfig::default()
        .max_message_size(Some(config.max_ws_message_bytes))
        .max_frame_size(Some(config.max_ws_message_bytes));
    let mut websocket = match tungstenite::accept_hdr_with_config(
        stream,
        callback,
        Some(websocket_config),
    ) {
        Ok(websocket) => websocket,
        Err(error) => {
            handshake_complete.store(true, Ordering::Release);
            tracing::debug!(connection = id, %peer, %error, "WebSocket handshake failed");
            return;
        }
    };
    handshake_complete.store(true, Ordering::Release);
    drop(websocket.get_ref().set_read_timeout(Some(READ_POLL)));

    let ingress = match ingress_receiver.recv() {
        Ok(Ok(ingress)) => ingress,
        Ok(Err(reason)) => {
            drop(websocket.close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: reason.into(),
            })));
            drop(websocket.flush());
            tracing::debug!(connection = id, %peer, "rejected WebSocket query selector");
            return;
        }
        Err(_) => {
            drop(websocket.close(Some(CloseFrame {
                code: CloseCode::Error,
                reason: "handshake metadata unavailable".into(),
            })));
            drop(websocket.flush());
            return;
        }
    };

    let (outbound, outbound_receiver) =
        mpsc::sync_channel::<OutboundCommand>(config.outbound_queue_messages);
    if events
        .send(ConnectionEvent::Connected {
            id,
            outbound,
            peer: peer.clone(),
            ingress,
        })
        .is_err()
    {
        return;
    }

    'connection: loop {
        loop {
            match outbound_receiver.try_recv() {
                Ok(OutboundCommand::Data {
                    message,
                    bytes,
                    version_frame,
                }) => {
                    let sent = websocket.send(message);
                    drop(events.send(ConnectionEvent::OutboundDrained {
                        id,
                        bytes,
                        version_frame,
                    }));
                    if sent.is_err() {
                        break 'connection;
                    }
                }
                Ok(OutboundCommand::Close { code, reason }) => {
                    drop(websocket.close(Some(CloseFrame {
                        code,
                        reason: reason.into(),
                    })));
                    drop(websocket.flush());
                    break 'connection;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    drop(websocket.close(Some(CloseFrame {
                        code: CloseCode::Away,
                        reason: "host connection state released".into(),
                    })));
                    drop(websocket.flush());
                    break 'connection;
                }
            }
        }

        match websocket.read() {
            Ok(Message::Text(text)) => {
                if events
                    .send(ConnectionEvent::Data {
                        id,
                        frame: DataFrame::Text(text.to_string()),
                        received_at: epoch.now(),
                    })
                    .is_err()
                {
                    break;
                }
            }
            Ok(Message::Binary(bytes)) => {
                if events
                    .send(ConnectionEvent::Data {
                        id,
                        frame: DataFrame::Binary(bytes.to_vec()),
                        received_at: epoch.now(),
                    })
                    .is_err()
                {
                    break;
                }
            }
            Ok(Message::Ping(_) | Message::Pong(_)) => {
                if events.send(ConnectionEvent::Control { id }).is_err() {
                    break;
                }
                drop(websocket.flush());
            }
            Ok(Message::Close(_) | Message::Frame(_)) => break,
            Err(tungstenite::Error::Io(error)) if is_transient_read(&error) => {}
            Err(error) => {
                tracing::debug!(connection = id, %peer, %error, "WebSocket connection ended");
                break;
            }
        }
    }
    drop(events.send(ConnectionEvent::Disconnected { id }));
}

fn spawn_handshake_watchdog(stream: &TcpStream, timeout: Duration, done: Arc<AtomicBool>) {
    let Ok(stream) = stream.try_clone() else {
        return;
    };
    let watchdog = thread::Builder::new()
        .name("ember-host-handshake-watchdog".to_string())
        .spawn(move || {
            let poll = Duration::from_millis(250);
            let started = std::time::Instant::now();
            while started.elapsed() < timeout {
                thread::sleep(poll);
                if done.load(Ordering::Acquire) {
                    return;
                }
            }
            drop(stream.shutdown(Shutdown::Both));
        });
    if let Err(error) = watchdog {
        tracing::warn!(%error, "handshake watchdog could not start");
    }
}

fn parse_ingress(
    query: Option<&str>,
    selectors: &BTreeMap<String, GameKey>,
) -> Result<Ingress, String> {
    let Some(query) = query else {
        return Ok(Ingress::Canonical);
    };
    let mut selected = None;
    for (name, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if name != "legacy_game" {
            continue;
        }
        if selected.is_some() {
            return Err("legacy_game may be supplied only once".to_string());
        }
        selected = Some(value.into_owned());
    }
    let Some(selector) = selected else {
        return Ok(Ingress::Canonical);
    };
    selectors
        .get(&selector)
        .cloned()
        .map(Ingress::Legacy)
        .ok_or_else(|| "unknown legacy_game selector".to_string())
}

fn is_transient_read(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_selector_is_closed_before_frames() {
        let selectors = BTreeMap::from([(
            "arena".to_string(),
            GameKey {
                game_id: "arena".to_string(),
                game_version: 12,
            },
        )]);
        assert!(matches!(
            parse_ingress(Some("legacy_game=arena"), &selectors),
            Ok(Ingress::Legacy(_))
        ));
        assert_eq!(
            parse_ingress(Some("legacy_game=unknown"), &selectors),
            Err("unknown legacy_game selector".to_string())
        );
        assert!(matches!(parse_ingress(None, &selectors), Ok(Ingress::Canonical)));
    }
}
