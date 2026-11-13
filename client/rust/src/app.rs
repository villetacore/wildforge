//! `WildforgeGame`, the root node of the client. It is a small state machine
//! (login screen <-> in game) that wires together networking, the session
//! data, the 3D world view, the UI and input.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use godot::classes::{INode3D, InputEvent, InputEventMouseButton, Node3D};
use godot::global::MouseButton;
use godot::prelude::*;
use serde_json::json;

use crate::content::ContentLibrary;
use crate::input::{Command, InputState};
use crate::net::{Catalog, Connection, NetEvent, ServerMsg};
use crate::session::Session;
use crate::ui::crafting::CraftingPanel;
use crate::ui::hud::Hud;
use crate::ui::login::LoginUi;
use crate::ui::{escape, UiEvent, UiQueue};
use crate::world::WorldView;

struct InGame {
    session: Session,
    world: WorldView,
    hud: Hud,
    crafting: CraftingPanel,
}

#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct WildforgeGame {
    base: Base<Node3D>,
    lib: Option<ContentLibrary>,
    queue: UiQueue,
    conn: Option<Connection>,
    connected: bool,
    pending: Vec<serde_json::Value>,
    login: Option<LoginUi>,
    game: Option<InGame>,
    input: InputState,
}

#[godot_api]
impl INode3D for WildforgeGame {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            lib: None,
            queue: Rc::new(RefCell::new(VecDeque::new())),
            conn: None,
            connected: false,
            pending: Vec::new(),
            login: None,
            game: None,
            input: InputState::default(),
        }
    }

    fn ready(&mut self) {
        self.lib = Some(ContentLibrary::new());
        self.show_login();

        // Dev helper: WILDFORGE_AUTOLOGIN="user:password[@ws://host:port/ws]"
        // registers (if needed) and logs in right away.
        if let Ok(spec) = std::env::var("WILDFORGE_AUTOLOGIN") {
            let (creds, server) = spec.split_once('@').unwrap_or((spec.as_str(), ""));
            if let (Some((user, pass)), Some(login)) = (creds.split_once(':'), &mut self.login) {
                login.username.set_text(user);
                login.password.set_text(pass);
                if !server.is_empty() {
                    login.server.set_text(server);
                }
                crate::ui::push(&self.queue, UiEvent::Register);
                crate::ui::push(&self.queue, UiEvent::Login);
            }
        }
    }

    fn process(&mut self, delta: f64) {
        let delta = delta as f32;
        self.poll_network();
        self.drain_ui();
        self.handle_input(delta);
        if let (Some(g), Some(lib)) = (&mut self.game, &self.lib) {
            g.world.update(delta, g.session.my_id);
            g.hud.update(&g.session, lib);
            g.crafting.update(&g.session, lib);
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        let Some(g) = &mut self.game else { return };
        if let Ok(mb) = event.try_cast::<InputEventMouseButton>() {
            if mb.is_pressed() {
                match mb.get_button_index() {
                    MouseButton::WHEEL_UP => g.world.camera.zoom(-1.0),
                    MouseButton::WHEEL_DOWN => g.world.camera.zoom(1.0),
                    _ => {}
                }
            }
        }
    }
}

impl WildforgeGame {
    // ------------------------------------------------------------ screens

    fn show_login(&mut self) {
        if let Some(g) = self.game.take() {
            let mut layer = g.hud.layer.clone();
            layer.queue_free();
            g.world.destroy();
        }
        if self.login.is_none() {
            let login = LoginUi::build(&self.queue);
            self.base_mut().add_child(&login.layer);
            let mut user = login.username.clone();
            user.grab_focus();
            self.login = Some(login);
        }
    }

    fn start_game(&mut self, session: Session, depleted: Vec<usize>) {
        if let Some(mut login) = self.login.take() {
            login.layer.queue_free();
        }
        let lib = self.lib.as_ref().unwrap();
        let mut base = self.base().clone().upcast::<Node3D>();
        let world = WorldView::new(&mut base, &session, &depleted, lib);
        let mut hud = Hud::build(&self.queue, session.minimap_texture());
        let crafting = CraftingPanel::build(&mut hud.layer, &self.queue);
        crafting.connect(&self.queue, &session);
        self.base_mut().add_child(&hud.layer);
        self.game = Some(InGame {
            session,
            world,
            hud,
            crafting,
        });
    }

    fn set_status(&mut self, text: &str, error: bool) {
        if let Some(login) = &mut self.login {
            login.set_status(text, error);
        }
    }

    fn chat_line(&mut self, bbcode: &str) {
        if let Some(g) = &mut self.game {
            g.hud.chat_line(bbcode);
        }
    }

    // ------------------------------------------------------------ network

    fn send(&mut self, v: serde_json::Value) {
        match &self.conn {
            Some(c) if self.connected => c.send(v),
            _ => self.pending.push(v),
        }
    }

    fn auth(&mut self, kind: &str) {
        let Some(login) = &self.login else { return };
        let (url, user, pass) = login.credentials();
        if user.is_empty() || pass.is_empty() {
            self.set_status("Enter a username and password.", true);
            return;
        }
        if self.conn.is_none() {
            self.connected = false;
            self.conn = Some(Connection::open(url));
            self.set_status("Connecting...", false);
        }
        self.send(json!({ "t": kind, "username": user, "password": pass }));
    }

    fn poll_network(&mut self) {
        let events = match &self.conn {
            Some(c) => c.poll(),
            None => return,
        };
        for ev in events {
            match ev {
                NetEvent::Connected => {
                    self.connected = true;
                    for m in std::mem::take(&mut self.pending) {
                        if let Some(c) = &self.conn {
                            c.send(m);
                        }
                    }
                }
                NetEvent::Disconnected(reason) => {
                    self.conn = None;
                    self.connected = false;
                    self.pending.clear();
                    self.show_login();
                    self.set_status(&reason, true);
                }
                NetEvent::Msg(m) => self.on_message(*m),
            }
        }
    }

    fn on_message(&mut self, m: ServerMsg) {
        match m {
            ServerMsg::Registered { username } => {
                self.set_status(
                    &format!("Account '{username}' created! Logging in..."),
                    false,
                );
                self.auth("login");
            }
            ServerMsg::Error { message } => {
                if self.game.is_some() {
                    self.chat_line(&format!("[color=#ff7766]{}[/color]", escape(&message)));
                } else {
                    self.set_status(&message, true);
                }
            }
            ServerMsg::Welcome {
                id,
                name,
                size,
                terrain,
                heights,
                nodes,
                depleted,
                content,
            } => {
                if self.game.is_none() {
                    let mut session =
                        Session::new(id, name, Catalog::new(content), size, &terrain, &nodes);
                    session.heights = heights;
                    self.start_game(session, depleted);
                }
            }
            ServerMsg::State {
                you,
                players,
                enemies,
                items,
                fx,
            } => {
                let (Some(g), Some(lib)) = (&mut self.game, &self.lib) else {
                    return;
                };
                g.session.you = you;
                let cat = &g.session.catalog;
                g.world
                    .entities
                    .sync_players(&players, g.session.my_id, cat, lib);
                g.world.entities.sync_enemies(&enemies, cat, lib);
                g.world.entities.sync_items(&items, cat, lib);
                g.world.play_fx(&fx, cat);
            }
            ServerMsg::Node { i, alive } => {
                if let Some(g) = &mut self.game {
                    g.world.set_node_alive(i, alive);
                }
            }
            ServerMsg::Chat { from, text } => {
                let me = self
                    .game
                    .as_ref()
                    .map(|g| g.session.my_name.clone())
                    .unwrap_or_default();
                let line = if from.is_empty() {
                    format!("[color=#f2d16b]{}[/color]", escape(&text))
                } else {
                    let color = if from == me { "#ffd27a" } else { "#8fc8ff" };
                    format!(
                        "[color={color}]{}:[/color] {}",
                        escape(&from),
                        escape(&text)
                    )
                };
                self.chat_line(&line);
            }
            ServerMsg::Died { lost_gold } => {
                self.chat_line(&format!(
                    "[color=#ff5544]You died and lost {lost_gold} gold. Respawned in town.[/color]"
                ));
            }
            ServerMsg::Unknown => {}
        }
    }

    // ----------------------------------------------------------- ui/input

    fn drain_ui(&mut self) {
        loop {
            let ev = self.queue.borrow_mut().pop_front();
            let Some(ev) = ev else { break };
            match ev {
                UiEvent::Login => self.auth("login"),
                UiEvent::Register => self.auth("register"),
                UiEvent::Chat(text) => {
                    if !text.trim().is_empty() {
                        self.send(json!({ "t": "chat", "text": text }));
                    }
                    if let Some(g) = &mut self.game {
                        g.hud.chat_input.clear();
                        g.hud.chat_input.release_focus();
                    }
                    self.input.chat_block = 0.2;
                }
                UiEvent::UseItem(i) => self.send(json!({ "t": "use", "index": i })),
                UiEvent::DropItem(i) => self.send(json!({ "t": "drop", "index": i })),
                UiEvent::Unequip(slot) => self.send(json!({ "t": "unequip", "slot": slot })),
                UiEvent::Craft(id) => self.send(json!({ "t": "craft", "recipe": id })),
                UiEvent::CraftSelected => {
                    if let Some(id) = self.game.as_ref().and_then(|g| g.crafting.selected()) {
                        self.send(json!({ "t": "craft", "recipe": id }));
                    }
                }
                UiEvent::ToggleCrafting => {
                    if let Some(g) = &mut self.game {
                        g.crafting.toggle();
                    }
                }
            }
        }
    }

    fn handle_input(&mut self, delta: f32) {
        let Some(g) = &mut self.game else { return };
        let commands = if g.hud.chat_focused() {
            let godot_input = godot::classes::Input::singleton();
            if godot_input.is_physical_key_pressed(godot::global::Key::ESCAPE) {
                g.hud.chat_input.release_focus();
            }
            self.input.release().into_iter().collect()
        } else {
            self.input.poll(delta)
        };
        for c in commands {
            let msg = match c {
                Command::Move(dx, dy) => json!({ "t": "move", "dx": dx, "dy": dy }),
                Command::Attack => json!({ "t": "attack" }),
                Command::Gather => json!({ "t": "gather" }),
                Command::Potion => json!({ "t": "potion" }),
                Command::UseSlot(i) => json!({ "t": "use", "index": i }),
                Command::ToggleCrafting => {
                    if let Some(g) = &mut self.game {
                        g.crafting.toggle();
                    }
                    continue;
                }
                Command::FocusChat => {
                    if let Some(g) = &mut self.game {
                        g.hud.chat_input.grab_focus();
                    }
                    continue;
                }
            };
            self.send(msg);
        }
    }
}
