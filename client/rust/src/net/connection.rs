//! WebSocket connection running on a background thread so the Godot main
//! thread never blocks on the network.

use std::io::ErrorKind;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Error, Message};

use super::protocol::ServerMsg;

pub enum NetEvent {
    Connected,
    Msg(Box<ServerMsg>),
    Disconnected(String),
}

pub struct Connection {
    out: Sender<String>,
    events: Receiver<NetEvent>,
}

impl Connection {
    pub fn open(url: String) -> Connection {
        let (out_tx, out_rx) = mpsc::channel::<String>();
        let (ev_tx, ev_rx) = mpsc::channel::<NetEvent>();
        thread::spawn(move || {
            let reason = run(&url, out_rx, &ev_tx);
            let _ = ev_tx.send(NetEvent::Disconnected(reason));
        });
        Connection {
            out: out_tx,
            events: ev_rx,
        }
    }

    pub fn send(&self, v: serde_json::Value) {
        let _ = self.out.send(v.to_string());
    }

    pub fn poll(&self) -> Vec<NetEvent> {
        self.events.try_iter().collect()
    }
}

fn run(url: &str, out: Receiver<String>, ev: &Sender<NetEvent>) -> String {
    let (mut ws, _) = match tungstenite::connect(url) {
        Ok(v) => v,
        Err(e) => return format!("Cannot connect to {url}: {e}"),
    };
    if let MaybeTlsStream::Plain(s) = ws.get_mut() {
        let _ = s.set_nodelay(true);
        // A short read timeout lets one thread both read and write.
        let _ = s.set_read_timeout(Some(Duration::from_millis(5)));
    }
    let _ = ev.send(NetEvent::Connected);
    loop {
        loop {
            match out.try_recv() {
                Ok(text) => {
                    if let Err(e) = ws.send(Message::text(text)) {
                        return format!("Connection lost: {e}");
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    return "Disconnected".into();
                }
            }
        }
        match ws.read() {
            Ok(Message::Text(t)) => match serde_json::from_str::<ServerMsg>(t.as_str()) {
                Ok(m) => {
                    if ev.send(NetEvent::Msg(Box::new(m))).is_err() {
                        return "Client closed".into();
                    }
                }
                Err(e) => eprintln!("bad server message: {e}"),
            },
            Ok(Message::Close(_)) => return "Server closed the connection".into(),
            Ok(_) => {}
            Err(Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(Error::ConnectionClosed) | Err(Error::AlreadyClosed) => {
                return "Server closed the connection".into()
            }
            Err(e) => return format!("Connection lost: {e}"),
        }
    }
}
