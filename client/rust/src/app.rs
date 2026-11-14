//! `WildforgeGame`, the root node of the client. It is a small state machine
//! (login screen <-> in game) that wires together networking, the session
//! data, the 3D world view, the UI and input.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use godot::classes::input::MouseMode;
use godot::classes::{
    INode3D, Input, InputEvent, InputEventMouseButton, InputEventMouseMotion, Node3D,
};
use godot::global::MouseButton;
use godot::prelude::*;
use serde_json::json;

use crate::content::{parse_color, ContentLibrary};
use crate::input::{Command, InputState};
use crate::net::{Connection, NetEvent, ServerMsg};
use crate::session::Session;
use crate::ui::crafting::CraftingPanel;
use crate::ui::dialog::DialogPanel;
use crate::ui::hud::Hud;
use crate::ui::login::LoginUi;
use crate::ui::{escape, UiEvent, UiQueue};
use crate::world::entities::{Pick, RANK_COLORS};
use crate::world::WorldView;

struct InGame {
    session: Session,
    world: WorldView,
    hud: Hud,
    crafting: CraftingPanel,
    dialog: DialogPanel,
    enemies: Vec<crate::net::protocol::EnemyState>,
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
    dragging: bool,
    drag_moved: f32,
    press_pos: Vector2,
    sent_dir: Vector2,
    script: Option<Script>,
}

/// Dev/docs helper: WILDFORGE_SCRIPT=<file> runs simple commands once in game
/// (wait s · shot path.png · chat text · yaw rad · pitch rad · zoom dist ·
/// menu name · talk opt · move dx dy · target nearest · quit).
struct Script {
    lines: Vec<String>,
    next: usize,
    wait: f32,
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
            dragging: false,
            drag_moved: 0.0,
            press_pos: Vector2::ZERO,
            sent_dir: Vector2::ZERO,
            script: std::env::var("WILDFORGE_SCRIPT")
                .ok()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|t| Script {
                    lines: t.lines().map(str::to_string).collect(),
                    next: 0,
                    wait: 0.0,
                }),
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
        self.update_mouse_mode();
        self.run_script(delta);
        if let (Some(g), Some(lib)) = (&mut self.game, &self.lib) {
            g.world.update(delta, &g.session);
            g.hud.update(&g.session, lib, g.world.camera.yaw);
            g.crafting.update(&g.session, lib);
        }
    }

    fn exit_tree(&mut self) {
        // Cached meshes must be released while the engine is still alive.
        crate::world::models::clear_cache();
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if self.game.is_none() {
            return;
        }
        let action_mode = self.action_mode();
        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            let pressed = mb.is_pressed();
            match mb.get_button_index() {
                MouseButton::WHEEL_UP if pressed => {
                    self.game.as_mut().unwrap().world.camera.zoom(-1.0)
                }
                MouseButton::WHEEL_DOWN if pressed => {
                    self.game.as_mut().unwrap().world.camera.zoom(1.0)
                }
                // Action mode: the mouse aims the camera, buttons attack.
                MouseButton::LEFT if action_mode && pressed => self.attack("light"),
                MouseButton::RIGHT if action_mode && pressed => self.attack("heavy"),
                // Cursor mode (Ctrl held or a window open): drag to orbit, click to pick.
                MouseButton::RIGHT | MouseButton::MIDDLE if !action_mode => {
                    self.dragging = pressed;
                    if pressed {
                        self.press_pos = mb.get_position();
                    }
                }
                MouseButton::LEFT if !action_mode => {
                    if pressed {
                        self.press_pos = mb.get_position();
                        self.drag_moved = 0.0;
                    } else if self.drag_moved < 6.0 {
                        self.click(mb.get_position());
                    }
                }
                _ => {}
            }
        } else if let Ok(mm) = event.try_cast::<InputEventMouseMotion>() {
            let rel = mm.get_relative();
            self.drag_moved += rel.length();
            if action_mode || self.dragging {
                let s = if action_mode { 0.8 } else { 1.0 };
                self.game
                    .as_mut()
                    .unwrap()
                    .world
                    .camera
                    .rotate(rel.x * s, rel.y * s);
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
        Input::singleton().set_mouse_mode(MouseMode::VISIBLE);
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
        let dialog = DialogPanel::build(&mut hud.layer, &self.queue);
        self.base_mut().add_child(&hud.layer);
        self.game = Some(InGame {
            session,
            world,
            hud,
            crafting,
            dialog,
            enemies: Vec::new(),
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

    /// Action mode captures the mouse for camera and attacks; it is off
    /// while Ctrl is held or any window wants the cursor.
    fn action_mode(&self) -> bool {
        let Some(g) = &self.game else { return false };
        let ctrl = Input::singleton().is_physical_key_pressed(godot::global::Key::CTRL);
        !(ctrl
            || g.dialog.visible()
            || g.crafting.visible()
            || g.hud.cursor_wanted()
            || g.hud.chat_focused())
    }

    fn update_mouse_mode(&mut self) {
        let want = if self.game.is_some() && self.action_mode() {
            MouseMode::CAPTURED
        } else {
            MouseMode::VISIBLE
        };
        let mut input = Input::singleton();
        if input.get_mouse_mode() != want {
            input.set_mouse_mode(want);
        }
    }

    /// Camera-forward direction on the ground, as server (x, y).
    fn aim(&self) -> Vector2 {
        self.game
            .as_ref()
            .map(|g| g.world.camera.basis_2d().0)
            .unwrap_or(Vector2::new(0.0, 1.0))
    }

    fn attack(&mut self, kind: &str) {
        let f = self.aim();
        self.send(json!({ "t": "attack", "opt": kind, "dx": f.x, "dy": f.y }));
    }

    /// Left click in the world: select an enemy (and attack it if it was
    /// already selected) or talk to an NPC.
    fn click(&mut self, pos: Vector2) {
        let Some(g) = &mut self.game else { return };
        let cam = g.world.camera.cam.clone();
        match g.world.entities.pick(&cam, pos) {
            Some(Pick::Enemy(id)) => {
                let again = g.world.entities.target == id;
                g.world.entities.target = id;
                self.send(json!({ "t": "target", "target": id }));
                if again {
                    self.send(json!({ "t": "attack" }));
                }
            }
            Some(Pick::Npc(id)) => self.send(json!({ "t": "target", "target": id })),
            _ => {}
        }
    }

    fn run_script(&mut self, delta: f32) {
        if self.game.is_none() {
            return;
        }
        let Some(sc) = &mut self.script else { return };
        if sc.wait > 0.0 {
            sc.wait -= delta;
            return;
        }
        let Some(line) = sc.lines.get(sc.next).cloned() else {
            return;
        };
        sc.next += 1;
        let (cmd, arg) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
        let num = |i: usize| {
            arg.split_whitespace()
                .nth(i)
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0)
        };
        match cmd {
            "wait" => sc.wait = num(0),
            "shot" => {
                if let Some(img) = self
                    .base()
                    .get_viewport()
                    .and_then(|v| v.get_texture())
                    .and_then(|t| t.get_image())
                {
                    img.save_png(arg);
                    godot_print!("wildforge: screenshot {arg}");
                }
            }
            "chat" => self.send(json!({ "t": "chat", "text": arg })),
            "menu" => self.send(json!({ "t": "menu", "menu": arg })),
            "talk" => self.talk(arg, None),
            "move" => {
                self.send(json!({ "t": "move", "dx": num(0), "dy": num(1) }));
            }
            "target" => self.next_target(),
            "attack" => self.attack(if arg == "heavy" { "heavy" } else { "light" }),
            "act" => {
                let (kind, opt) = arg.split_once(' ').unwrap_or((arg, ""));
                self.send(json!({ "t": kind, "opt": opt }));
            }
            "yaw" | "pitch" | "zoom" => {
                if let Some(g) = &mut self.game {
                    g.world.camera.set_view(cmd, num(0));
                }
            }
            "quit" => {
                self.base().get_tree().quit();
            }
            _ => {}
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
            ServerMsg::Welcome(w) => {
                if self.game.is_none() {
                    self.set_status("Building the world...", false);
                    let depleted = w.depleted.clone();
                    let session = Session::new(*w);
                    self.start_game(session, depleted);
                }
            }
            ServerMsg::State {
                you,
                players,
                enemies,
                pets,
                items,
                fx,
            } => {
                let (Some(g), Some(lib)) = (&mut self.game, &self.lib) else {
                    return;
                };
                g.session.set_you(*you);
                g.world.entities.target = g.session.you.target;
                let cat = &g.session.catalog;
                g.world
                    .entities
                    .sync_players(&players, g.session.my_id, cat, lib);
                g.world.entities.sync_enemies(&enemies, lib);
                g.world.entities.sync_pets(&pets, lib);
                g.world.entities.sync_items(&items, cat, lib);
                g.world.play_fx(&fx, cat);
                let target = enemies
                    .iter()
                    .find(|e| e.id == g.session.you.target)
                    .map(|e| {
                        (
                            format!("{} — level {}", e.name, e.level),
                            e.hp,
                            e.max_hp,
                            parse_color(RANK_COLORS[e.rank.clamp(0, 5) as usize]),
                        )
                    });
                g.hud.set_target(target);
                g.enemies = enemies;
            }
            ServerMsg::Node { i, alive } => {
                if let Some(g) = &mut self.game {
                    g.world.set_node_alive(i, alive);
                }
            }
            ServerMsg::Dungeon {
                index,
                x,
                y,
                size,
                terrain,
                portals,
            } => {
                if let (Some(g), Some(lib)) = (&mut self.game, &self.lib) {
                    g.session.apply_dungeon(x, y, size, &terrain, portals);
                    g.world.rebuild_dungeon(
                        index,
                        g.session.size,
                        &g.session.terrain,
                        &g.session.portals,
                        lib,
                    );
                }
            }
            ServerMsg::Dialog(d) => {
                if let Some(g) = &mut self.game {
                    g.dialog.show(&d);
                }
            }
            ServerMsg::DialogClose => {
                if let Some(g) = &mut self.game {
                    g.dialog.hide();
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
                self.chat_line(&format!("[color=#ff5544]You died and lost {lost_gold} gold. Respawned in Velhrad.[/color]"));
            }
            ServerMsg::Unknown => {}
        }
    }

    // ----------------------------------------------------------- ui/input

    fn talk(&mut self, opt: &str, text: Option<String>) {
        let Some(g) = &self.game else { return };
        let (npc, menu) = (g.dialog.npc, g.dialog.menu.clone());
        let mut msg = json!({ "t": "talk", "npc": npc, "opt": opt, "menu": menu });
        if let Some(t) = text {
            msg["text"] = json!(t);
        }
        self.send(msg);
    }

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
                UiEvent::Cast(i) => self.send(json!({ "t": "cast", "spell": i })),
                UiEvent::Menu(m) => self.send(json!({ "t": "menu", "menu": m })),
                UiEvent::DialogOption(id) => self.talk(&id, None),
                UiEvent::DialogSubmit(text) => {
                    self.talk("submit", Some(text));
                    self.input.chat_block = 0.2;
                }
                UiEvent::DialogClose => {
                    if let Some(g) = &mut self.game {
                        g.dialog.hide();
                    }
                    self.send(json!({ "t": "close" }));
                }
            }
        }
    }

    fn next_target(&mut self) {
        let Some(g) = &mut self.game else { return };
        let me = (g.session.you.x, g.session.you.y);
        let mut list: Vec<(f32, i64)> = g
            .enemies
            .iter()
            .map(|e| (((e.x - me.0).powi(2) + (e.y - me.1).powi(2)).sqrt(), e.id))
            .filter(|(d, _)| *d < 20.0)
            .collect();
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        if list.is_empty() {
            return;
        }
        let cur = list
            .iter()
            .position(|(_, id)| *id == g.world.entities.target);
        let next = list[cur.map(|i| (i + 1) % list.len()).unwrap_or(0)].1;
        g.world.entities.target = next;
        self.send(json!({ "t": "target", "target": next }));
    }

    fn handle_input(&mut self, delta: f32) {
        let Some(g) = &mut self.game else { return };
        let typing = g.hud.chat_focused() || g.dialog.prompt_focused();
        let commands: Vec<Command> = if typing {
            let godot_input = Input::singleton();
            if godot_input.is_physical_key_pressed(godot::global::Key::ESCAPE) {
                g.hud.chat_input.release_focus();
                g.dialog.prompt.release_focus();
            }
            self.input.release().into_iter().collect()
        } else {
            self.input.poll(delta)
        };
        // Camera-relative movement, re-sent when the camera turns.
        let (right, fwd) = self.input.direction();
        let (f, r) = g.world.camera.basis_2d();
        let mut dir = f * fwd as f32 + r * right as f32;
        if dir.length() > 0.01 {
            dir = dir.normalized();
        }
        let changed = (dir - self.sent_dir).length() > 0.06
            || (dir.length() < 0.01) != (self.sent_dir.length() < 0.01);
        if changed {
            self.sent_dir = dir;
            self.send(json!({ "t": "move", "dx": dir.x, "dy": dir.y }));
        }
        for c in commands {
            let msg = match c {
                Command::Move => continue,
                Command::Jump => json!({ "t": "jump" }),
                Command::Dodge => {
                    let d = self.sent_dir;
                    json!({ "t": "dodge", "dx": d.x, "dy": d.y })
                }
                Command::Block(on) => json!({ "t": "block", "opt": if on { "on" } else { "off" } }),
                Command::Sprint(on) => {
                    json!({ "t": "sprint", "opt": if on { "on" } else { "off" } })
                }
                Command::Gather => json!({ "t": "gather" }),
                Command::Potion => json!({ "t": "potion" }),
                Command::Tame => json!({ "t": "tame" }),
                Command::UseSlot(i) => json!({ "t": "use", "index": i }),
                Command::Cast(i) => json!({ "t": "cast", "spell": i }),
                Command::Menu(m) => json!({ "t": "menu", "menu": m }),
                Command::NextTarget => {
                    self.next_target();
                    continue;
                }
                Command::ToggleCrafting => {
                    if let Some(g) = &mut self.game {
                        g.crafting.toggle();
                    }
                    continue;
                }
                Command::ToggleInventory => {
                    if let Some(g) = &mut self.game {
                        g.hud.toggle_inventory();
                    }
                    continue;
                }
                Command::ToggleMap => {
                    if let Some(g) = &mut self.game {
                        g.hud.toggle_map();
                    }
                    continue;
                }
                Command::Escape => {
                    if let Some(g) = &mut self.game {
                        if g.dialog.visible() {
                            g.dialog.hide();
                            self.send(json!({ "t": "close" }));
                        } else if !g.hud.close_map() {
                            g.world.entities.target = 0;
                            self.send(json!({ "t": "target", "target": 0 }));
                        }
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
