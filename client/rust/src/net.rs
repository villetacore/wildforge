//! WebSocket connection to the Go server, run on a background thread so the
//! Godot main thread never blocks on the network.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Error, Message};

#[derive(Deserialize, Debug, Clone)]
pub struct CatalogEntry {
    pub name: String,
    pub desc: String,
    pub slot: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct You {
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
    pub xp: i32,
    pub xp_next: i32,
    pub gold: i32,
    pub attack: i32,
    pub defense: i32,
    pub inventory: Vec<String>,
    pub weapon: String,
    pub armor: String,
    pub kills: i32,
    pub online: i32,
}

#[derive(Deserialize, Debug)]
pub struct PlayerState {
    pub id: i64,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
}

#[derive(Deserialize, Debug)]
pub struct EnemyState {
    pub id: i64,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
}

#[derive(Deserialize, Debug)]
pub struct ItemState {
    pub id: i64,
    pub item: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize, Debug)]
pub struct Fx {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub text: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    Registered { username: String },
    Error { message: String },
    Welcome { id: i64, name: String, w: usize, h: usize, map: String, items: HashMap<String, CatalogEntry> },
    State { you: You, players: Vec<PlayerState>, enemies: Vec<EnemyState>, items: Vec<ItemState>, fx: Vec<Fx> },
    Chat { from: String, text: String },
    Died { lost_gold: i32 },
    #[serde(other)]
    Unknown,
}

pub enum NetEvent {
    Connected,
    Msg(ServerMsg),
    Disconnected(String),
}

pub struct Net {
    out: Sender<String>,
    events: Receiver<NetEvent>,
}

impl Net {
    pub fn connect(url: String) -> Net {
        let (out_tx, out_rx) = mpsc::channel::<String>();
        let (ev_tx, ev_rx) = mpsc::channel::<NetEvent>();
        thread::spawn(move || {
            let reason = run(&url, out_rx, &ev_tx);
            let _ = ev_tx.send(NetEvent::Disconnected(reason));
        });
        Net { out: out_tx, events: ev_rx }
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
        // Short read timeout lets one thread both read and write.
        let _ = TcpStream::set_read_timeout(s, Some(Duration::from_millis(5)));
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
                    if ev.send(NetEvent::Msg(m)).is_err() {
                        return "Client closed".into();
                    }
                }
                Err(e) => eprintln!("bad server message: {e}"),
            },
            Ok(Message::Close(_)) => return "Server closed the connection".into(),
            Ok(_) => {}
            Err(Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(Error::ConnectionClosed) | Err(Error::AlreadyClosed) => return "Server closed the connection".into(),
            Err(e) => return format!("Connection lost: {e}"),
        }
    }
}
