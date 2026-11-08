//! `WildforgeGame`: the root node of the client. Owns the network connection,
//! the UI, the local copy of the world and draws everything with `_draw`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::{PI, TAU};
use std::rc::Rc;

use godot::classes::canvas_item::TextureFilter;
use godot::classes::image::Format;
use godot::classes::{Camera2D, Font, INode2D, Image, ImageTexture, Input, Node2D, ThemeDb};
use godot::global::{HorizontalAlignment, Key};
use godot::prelude::*;
use serde_json::json;

use crate::net::{CatalogEntry, Net, NetEvent, ServerMsg, You};
use crate::ui::{self, Hud, LoginUi, UiEvent, UiQueue};

const TILE: f32 = 32.0;

fn tile_color(t: u8) -> Color {
    match t {
        0 => Color::from_rgb(0.36, 0.62, 0.30), // grass
        1 => Color::from_rgb(0.18, 0.38, 0.72), // water
        2 => Color::from_rgb(0.86, 0.80, 0.55), // sand
        3 => Color::from_rgb(0.20, 0.44, 0.20), // tree (ground under it)
        4 => Color::from_rgb(0.48, 0.48, 0.50), // stone
        5 => Color::from_rgb(0.25, 0.49, 0.23), // forest floor
        6 => Color::from_rgb(0.62, 0.52, 0.36), // road
        _ => Color::MAGENTA,
    }
}

fn enemy_look(kind: &str) -> (Color, f32) {
    match kind {
        "slime" => (Color::from_rgb(0.45, 0.88, 0.35), 9.0),
        "wolf" => (Color::from_rgb(0.55, 0.55, 0.62), 11.0),
        "skeleton" => (Color::from_rgb(0.92, 0.90, 0.80), 11.0),
        "ogre" => (Color::from_rgb(0.58, 0.36, 0.52), 16.0),
        _ => (Color::from_rgb(1.0, 0.0, 1.0), 10.0),
    }
}

#[derive(Default)]
struct Ent {
    pos: Vector2,
    target: Vector2,
    facing: Vector2,
    name: String,
    kind: String,
    hp: f32,
    max_hp: f32,
    level: i32,
}

impl Ent {
    fn update(&mut self, x: f32, y: f32, fresh: bool) {
        let t = Vector2::new(x * TILE, y * TILE);
        if fresh {
            self.pos = t;
        }
        let d = t - self.target;
        if d.length() > 0.5 {
            self.facing = d.normalized();
        }
        self.target = t;
    }
}

struct Floater {
    pos: Vector2,
    text: String,
    color: Color,
    t: f32,
}

#[derive(PartialEq)]
enum Mode {
    Login,
    Playing,
}

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct WildforgeGame {
    base: Base<Node2D>,
    queue: UiQueue,
    net: Option<Net>,
    connected: bool,
    pending: Vec<serde_json::Value>,
    mode: Mode,
    login: Option<LoginUi>,
    hud: Option<Hud>,
    camera: Option<Gd<Camera2D>>,
    font: Option<Gd<Font>>,

    my_id: i64,
    my_name: String,
    map: Vec<u8>,
    map_w: usize,
    map_h: usize,
    map_tex: Option<Gd<ImageTexture>>,
    catalog: HashMap<String, CatalogEntry>,
    you: You,
    shown_inventory: Vec<String>,
    players: HashMap<i64, Ent>,
    enemies: HashMap<i64, Ent>,
    items: HashMap<i64, Ent>,
    floaters: Vec<Floater>,
    swings: Vec<(Vector2, f32)>,

    last_dir: (i32, i32),
    keys_down: HashSet<i32>,
    attack_repeat: f32,
    chat_refocus_block: f32,
}

#[godot_api]
impl INode2D for WildforgeGame {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            queue: Rc::new(RefCell::new(VecDeque::new())),
            net: None,
            connected: false,
            pending: Vec::new(),
            mode: Mode::Login,
            login: None,
            hud: None,
            camera: None,
            font: None,
            my_id: 0,
            my_name: String::new(),
            map: Vec::new(),
            map_w: 0,
            map_h: 0,
            map_tex: None,
            catalog: HashMap::new(),
            you: You::default(),
            shown_inventory: Vec::new(),
            players: HashMap::new(),
            enemies: HashMap::new(),
            items: HashMap::new(),
            floaters: Vec::new(),
            swings: Vec::new(),
            last_dir: (0, 0),
            keys_down: HashSet::new(),
            attack_repeat: 0.0,
            chat_refocus_block: 0.0,
        }
    }

    fn ready(&mut self) {
        self.font = ThemeDb::singleton().get_fallback_font();
        self.base_mut().set_texture_filter(TextureFilter::NEAREST);

        let mut cam = Camera2D::new_alloc();
        cam.set_position_smoothing_enabled(true);
        cam.set_position_smoothing_speed(10.0);
        self.base_mut().add_child(&cam);
        self.camera = Some(cam);

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
                let mut q = self.queue.borrow_mut();
                q.push_back(UiEvent::Register);
                q.push_back(UiEvent::Login);
            }
        }
    }

    fn process(&mut self, delta: f64) {
        let delta = delta as f32;
        self.poll_network();
        self.drain_ui();
        if self.mode == Mode::Playing {
            self.handle_input(delta);
            self.animate(delta);
            self.update_hud();
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        if self.mode != Mode::Playing {
            return;
        }
        self.draw_world();
    }
}

impl WildforgeGame {
    // -------------------------------------------------------------- screens

    fn show_login(&mut self) {
        if let Some(mut hud) = self.hud.take() {
            hud.layer.queue_free();
        }
        let login = ui::build_login(&self.queue);
        self.base_mut().add_child(&login.layer);
        let mut user = login.username.clone();
        user.grab_focus();
        self.login = Some(login);
        self.mode = Mode::Login;
        self.players.clear();
        self.enemies.clear();
        self.items.clear();
        self.shown_inventory.clear();
    }

    fn show_game(&mut self) {
        if let Some(mut login) = self.login.take() {
            login.layer.queue_free();
        }
        let mut hud = ui::build_hud(&self.queue);
        self.base_mut().add_child(&hud.layer);
        if let Some(tex) = &self.map_tex {
            hud.minimap.set_texture(tex);
        }
        self.hud = Some(hud);
        self.mode = Mode::Playing;
    }

    fn set_status(&mut self, text: &str, error: bool) {
        if let Some(login) = &mut self.login {
            login.status.set_text(text);
            let c = if error { Color::from_rgb(1.0, 0.45, 0.4) } else { Color::from_rgb(0.6, 1.0, 0.6) };
            login.status.add_theme_color_override("font_color", c);
        }
    }

    // -------------------------------------------------------------- network

    fn send(&mut self, v: serde_json::Value) {
        match &self.net {
            Some(n) if self.connected => n.send(v),
            _ => self.pending.push(v),
        }
    }

    fn auth(&mut self, kind: &str) {
        let Some(login) = &self.login else { return };
        let mut url = login.server.get_text().to_string().trim().to_string();
        let user = login.username.get_text().to_string().trim().to_string();
        let pass = login.password.get_text().to_string();
        if user.is_empty() || pass.is_empty() {
            self.set_status("Enter a username and password.", true);
            return;
        }
        if !url.contains("://") {
            url = format!("ws://{url}");
        }
        if url.matches('/').count() < 3 {
            url.push_str("/ws");
        }
        if self.net.is_none() {
            self.connected = false;
            self.net = Some(Net::connect(url));
            self.set_status("Connecting...", false);
        }
        self.my_name = user.clone();
        self.send(json!({ "t": kind, "username": user, "password": pass }));
    }

    fn poll_network(&mut self) {
        let events = match &self.net {
            Some(n) => n.poll(),
            None => return,
        };
        for ev in events {
            match ev {
                NetEvent::Connected => {
                    self.connected = true;
                    let pending = std::mem::take(&mut self.pending);
                    if let Some(n) = &self.net {
                        for m in pending {
                            n.send(m);
                        }
                    }
                }
                NetEvent::Disconnected(reason) => {
                    self.net = None;
                    self.connected = false;
                    self.pending.clear();
                    if self.mode == Mode::Playing {
                        self.show_login();
                    }
                    self.set_status(&reason, true);
                }
                NetEvent::Msg(m) => self.on_message(m),
            }
        }
    }

    fn on_message(&mut self, m: ServerMsg) {
        match m {
            ServerMsg::Registered { username } => {
                self.set_status(&format!("Account '{username}' created! Logging in..."), false);
                self.auth("login");
            }
            ServerMsg::Error { message } => {
                if self.mode == Mode::Login {
                    self.set_status(&message, true);
                } else {
                    self.chat_line(&format!("[color=#ff7766]{}[/color]", escape(&message)));
                }
            }
            ServerMsg::Welcome { id, name, w, h, map, items } => {
                self.my_id = id;
                self.my_name = name;
                self.map_w = w;
                self.map_h = h;
                self.map = map.bytes().map(|b| b.saturating_sub(b'0')).collect();
                self.catalog = items;
                self.build_map_texture();
                self.show_game();
            }
            ServerMsg::State { you, players, enemies, items, fx } => {
                self.you = you;
                let mut seen = HashSet::new();
                for p in players {
                    seen.insert(p.id);
                    let fresh = !self.players.contains_key(&p.id);
                    let e = self.players.entry(p.id).or_default();
                    e.update(p.x, p.y, fresh);
                    (e.name, e.hp, e.max_hp, e.level) = (p.name, p.hp, p.max_hp, p.level);
                }
                self.players.retain(|id, _| seen.contains(id));
                seen.clear();
                for en in enemies {
                    seen.insert(en.id);
                    let fresh = !self.enemies.contains_key(&en.id);
                    let e = self.enemies.entry(en.id).or_default();
                    e.update(en.x, en.y, fresh);
                    (e.kind, e.hp, e.max_hp, e.level) = (en.kind, en.hp, en.max_hp, en.level);
                }
                self.enemies.retain(|id, _| seen.contains(id));
                seen.clear();
                for it in items {
                    seen.insert(it.id);
                    let e = self.items.entry(it.id).or_default();
                    e.update(it.x, it.y, true);
                    e.kind = it.item;
                }
                self.items.retain(|id, _| seen.contains(id));
                for f in fx {
                    let pos = Vector2::new(f.x * TILE, f.y * TILE);
                    let color = match f.kind.as_str() {
                        "swing" => {
                            self.swings.push((pos, 0.0));
                            continue;
                        }
                        "hit" => Color::from_rgb(1.0, 1.0, 1.0),
                        "hurt" => Color::from_rgb(1.0, 0.35, 0.3),
                        "heal" => Color::from_rgb(0.4, 1.0, 0.4),
                        "xp" => Color::from_rgb(0.5, 0.75, 1.0),
                        "gold" => Color::from_rgb(1.0, 0.85, 0.2),
                        "level" => Color::from_rgb(1.0, 0.9, 0.3),
                        _ => Color::from_rgb(0.9, 0.9, 0.9),
                    };
                    self.floaters.push(Floater { pos: pos + Vector2::new(0.0, -20.0), text: f.text, color, t: 0.0 });
                }
            }
            ServerMsg::Chat { from, text } => {
                let line = if from.is_empty() {
                    format!("[color=#f2d16b]{}[/color]", escape(&text))
                } else {
                    let color = if from == self.my_name { "#ffd27a" } else { "#8fc8ff" };
                    format!("[color={color}]{}:[/color] {}", escape(&from), escape(&text))
                };
                self.chat_line(&line);
            }
            ServerMsg::Died { lost_gold } => {
                self.chat_line(&format!("[color=#ff5544]You died and lost {lost_gold} gold. Respawned in town.[/color]"));
            }
            ServerMsg::Unknown => {}
        }
    }

    fn chat_line(&mut self, bbcode: &str) {
        if let Some(hud) = &mut self.hud {
            hud.chat_log.append_text(&format!("{bbcode}\n"));
        }
    }

    fn build_map_texture(&mut self) {
        let mut data = PackedByteArray::new();
        data.resize(self.map_w * self.map_h * 3);
        let bytes = data.as_mut_slice();
        for (i, t) in self.map.iter().enumerate() {
            let c = tile_color(*t);
            let c = if *t == 3 { Color::from_rgb(0.12, 0.32, 0.13) } else { c }; // trees darker on minimap
            bytes[i * 3] = (c.r * 255.0) as u8;
            bytes[i * 3 + 1] = (c.g * 255.0) as u8;
            bytes[i * 3 + 2] = (c.b * 255.0) as u8;
        }
        let img = Image::create_from_data(self.map_w as i32, self.map_h as i32, false, Format::RGB8, &data);
        self.map_tex = ImageTexture::create_from_image(img.as_ref());
    }

    // ------------------------------------------------------------------- ui

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
                    if let Some(hud) = &mut self.hud {
                        hud.chat_input.clear();
                        hud.chat_input.release_focus();
                    }
                    self.chat_refocus_block = 0.2;
                }
                UiEvent::UseItem(i) => self.send(json!({ "t": "use", "index": i })),
                UiEvent::DropItem(i) => self.send(json!({ "t": "drop", "index": i })),
                UiEvent::Unequip(slot) => self.send(json!({ "t": "unequip", "slot": slot })),
            }
        }
    }

    fn chat_focused(&self) -> bool {
        self.hud.as_ref().is_some_and(|h| h.chat_input.has_focus())
    }

    /// True only on the frame the key goes down.
    fn just_pressed(&mut self, input: &Gd<Input>, key: Key) -> bool {
        let down = input.is_physical_key_pressed(key);
        let code = key.ord();
        if down {
            self.keys_down.insert(code)
        } else {
            self.keys_down.remove(&code);
            false
        }
    }

    fn handle_input(&mut self, delta: f32) {
        let input = Input::singleton();
        self.chat_refocus_block -= delta;
        self.attack_repeat -= delta;

        if self.chat_focused() {
            if self.last_dir != (0, 0) {
                self.last_dir = (0, 0);
                self.send(json!({ "t": "move", "dx": 0, "dy": 0 }));
            }
            if input.is_physical_key_pressed(Key::ESCAPE) {
                if let Some(hud) = &mut self.hud {
                    hud.chat_input.release_focus();
                }
            }
            self.keys_down.clear();
            return;
        }

        let enter = self.just_pressed(&input, Key::ENTER);
        if enter && self.chat_refocus_block <= 0.0 {
            if let Some(hud) = &mut self.hud {
                hud.chat_input.grab_focus();
            }
            return;
        }

        let k = |key: Key| input.is_physical_key_pressed(key) as i32;
        let dx = (k(Key::D) | k(Key::RIGHT)) - (k(Key::A) | k(Key::LEFT));
        let dy = (k(Key::S) | k(Key::DOWN)) - (k(Key::W) | k(Key::UP));
        if (dx, dy) != self.last_dir {
            self.last_dir = (dx, dy);
            self.send(json!({ "t": "move", "dx": dx, "dy": dy }));
        }

        if input.is_physical_key_pressed(Key::SPACE) && self.attack_repeat <= 0.0 {
            self.attack_repeat = 0.25;
            self.send(json!({ "t": "attack" }));
        }
        if self.just_pressed(&input, Key::Q) {
            self.send(json!({ "t": "potion" }));
        }
        let digits = [Key::KEY_1, Key::KEY_2, Key::KEY_3, Key::KEY_4, Key::KEY_5, Key::KEY_6, Key::KEY_7, Key::KEY_8, Key::KEY_9];
        for (i, key) in digits.into_iter().enumerate() {
            if self.just_pressed(&input, key) {
                self.send(json!({ "t": "use", "index": i }));
            }
        }
    }

    fn animate(&mut self, delta: f32) {
        let a = 1.0 - (-delta * 14.0).exp();
        for e in self.players.values_mut().chain(self.enemies.values_mut()) {
            e.pos = e.pos.lerp(e.target, a);
        }
        for f in &mut self.floaters {
            f.t += delta;
            f.pos.y -= 28.0 * delta;
        }
        self.floaters.retain(|f| f.t < 1.2);
        for s in &mut self.swings {
            s.1 += delta;
        }
        self.swings.retain(|s| s.1 < 0.2);

        if let (Some(cam), Some(me)) = (&mut self.camera, self.players.get(&self.my_id)) {
            cam.set_position(me.pos);
        }
    }

    fn update_hud(&mut self) {
        let you = self.you.clone();
        let item_name = |id: &str| self.catalog.get(id).map(|c| c.name.clone()).unwrap_or_else(|| id.to_string());
        let weapon = if you.weapon.is_empty() { "-".to_string() } else { item_name(&you.weapon) };
        let armor = if you.armor.is_empty() { "-".to_string() } else { item_name(&you.armor) };
        let inv_changed = you.inventory != self.shown_inventory;
        let inv_rows: Vec<(String, String)> = you
            .inventory
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let c = self.catalog.get(id);
                let name = c.map(|c| c.name.as_str()).unwrap_or(id);
                let prefix = if i < 9 { format!("{}. ", i + 1) } else { "    ".into() };
                (format!("{prefix}{name}"), c.map(|c| c.desc.clone()).unwrap_or_default())
            })
            .collect();
        let (mw, mh) = (self.map_w.max(1) as f32, self.map_h.max(1) as f32);
        let Some(hud) = &mut self.hud else { return };

        hud.name.set_text(&format!("{}  ·  Level {}", self.my_name, you.level));
        hud.hp_label.set_text(&format!("HP {} / {}", you.hp, you.max_hp));
        hud.hp.set_max(you.max_hp.max(1.0) as f64);
        hud.hp.set_value(you.hp as f64);
        hud.xp_label.set_text(&format!("XP {} / {}", you.xp, you.xp_next));
        hud.xp.set_max(you.xp_next.max(1) as f64);
        hud.xp.set_value(you.xp as f64);
        hud.stats.set_text(&format!(
            "ATK {}   DEF {}   Gold {}   Kills {}   Online {}",
            you.attack, you.defense, you.gold, you.kills, you.online
        ));
        hud.equip.set_text(&format!("Weapon: {weapon}\nArmor: {armor}"));
        hud.marker.set_position(Vector2::new(you.x / mw * 200.0 - 2.5, you.y / mh * 200.0 - 2.5));

        if inv_changed {
            hud.inventory.clear();
            for (i, (text, tip)) in inv_rows.iter().enumerate() {
                hud.inventory.add_item(text);
                hud.inventory.set_item_tooltip(i as i32, tip);
            }
            self.shown_inventory = you.inventory;
        }
    }

    // ---------------------------------------------------------------- draw

    fn text(&mut self, pos: Vector2, text: &str, size: i32, color: Color) {
        let Some(font) = self.font.clone() else { return };
        let w = font.get_string_size_ex(text).font_size(size).done().x;
        // dark outline for readability
        let shadow = Color::from_rgba(0.0, 0.0, 0.0, color.a * 0.8);
        self.base()
            .draw_string_ex(&font, pos + Vector2::new(-w / 2.0 + 1.0, 1.0), text)
            .font_size(size)
            .modulate(shadow)
            .done();
        self.base()
            .draw_string_ex(&font, pos + Vector2::new(-w / 2.0, 0.0), text)
            .font_size(size)
            .alignment(HorizontalAlignment::LEFT)
            .modulate(color)
            .done();
    }

    fn hp_bar(&mut self, center: Vector2, width: f32, hp: f32, max_hp: f32, color: Color) {
        let frac = (hp / max_hp.max(1.0)).clamp(0.0, 1.0);
        let r = Rect2::new(center - Vector2::new(width / 2.0, 0.0), Vector2::new(width, 4.0));
        let mut b = self.base_mut();
        b.draw_rect(r.grow(1.0), Color::from_rgba(0.0, 0.0, 0.0, 0.7));
        b.draw_rect(Rect2::new(r.position, Vector2::new(width * frac, 4.0)), color);
    }

    fn draw_world(&mut self) {
        let cam_pos = self.camera.as_ref().map(|c| c.get_screen_center_position()).unwrap_or_default();
        let view = self.base().get_viewport_rect().size;
        let (w, h) = (self.map_w as i32, self.map_h as i32);

        // Ground: the whole map as one texture, one texel per tile.
        if let Some(tex) = self.map_tex.clone() {
            let size = Vector2::new(w as f32 * TILE, h as f32 * TILE);
            // Ocean beyond the edges.
            self.base_mut().draw_rect(
                Rect2::new(Vector2::new(-2000.0, -2000.0), size + Vector2::new(4000.0, 4000.0)),
                tile_color(1),
            );
            self.base_mut().draw_texture_rect(&tex, Rect2::new(Vector2::ZERO, size), false);
        }

        // Decorations on visible tiles only.
        let x0 = ((cam_pos.x - view.x / 2.0) / TILE).floor() as i32 - 1;
        let y0 = ((cam_pos.y - view.y / 2.0) / TILE).floor() as i32 - 1;
        let x1 = ((cam_pos.x + view.x / 2.0) / TILE).ceil() as i32 + 1;
        let y1 = ((cam_pos.y + view.y / 2.0) / TILE).ceil() as i32 + 1;
        for ty in y0.max(0)..y1.min(h) {
            for tx in x0.max(0)..x1.min(w) {
                let t = self.map[(ty * w + tx) as usize];
                let o = Vector2::new(tx as f32 * TILE, ty as f32 * TILE);
                let c = o + Vector2::new(TILE / 2.0, TILE / 2.0);
                let mut b = self.base_mut();
                match t {
                    3 => {
                        b.draw_circle(c + Vector2::new(2.0, 4.0), 13.0, Color::from_rgba(0.0, 0.0, 0.0, 0.25));
                        b.draw_circle(c, 13.0, Color::from_rgb(0.13, 0.36, 0.15));
                        b.draw_circle(c + Vector2::new(-3.0, -3.0), 8.0, Color::from_rgb(0.18, 0.47, 0.2));
                    }
                    4 => {
                        b.draw_rect(Rect2::new(o + Vector2::new(2.0, 2.0), Vector2::new(28.0, 28.0)), Color::from_rgb(0.38, 0.38, 0.40));
                        b.draw_rect(Rect2::new(o + Vector2::new(5.0, 4.0), Vector2::new(18.0, 12.0)), Color::from_rgb(0.58, 0.58, 0.6));
                    }
                    1 => {
                        if (tx * 7 + ty * 13) % 11 == 0 {
                            b.draw_line(c + Vector2::new(-6.0, 0.0), c + Vector2::new(6.0, 0.0), Color::from_rgba(1.0, 1.0, 1.0, 0.25));
                        }
                    }
                    0 => {
                        if (tx * 31 + ty * 17) % 9 == 0 {
                            b.draw_circle(c + Vector2::new(-5.0, 3.0), 2.0, Color::from_rgb(0.95, 0.9, 0.4));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Ground items.
        let items: Vec<(Vector2, String)> = self.items.values().map(|e| (e.pos, e.kind.clone())).collect();
        for (p, kind) in items {
            let slot = self.catalog.get(&kind).map(|c| c.slot.clone()).unwrap_or_default();
            let mut b = self.base_mut();
            match slot.as_str() {
                "gold" => {
                    b.draw_circle(p, 5.0, Color::from_rgb(0.75, 0.55, 0.1));
                    b.draw_circle(p, 3.5, Color::from_rgb(1.0, 0.85, 0.2));
                }
                "potion" => {
                    b.draw_circle(p, 6.0, Color::from_rgb(0.85, 0.15, 0.2));
                    b.draw_circle(p + Vector2::new(-2.0, -2.0), 2.0, Color::from_rgb(1.0, 0.7, 0.7));
                }
                "weapon" => {
                    let pts = PackedVector2Array::from(&[p + Vector2::new(0.0, -9.0), p + Vector2::new(6.0, 0.0), p + Vector2::new(0.0, 9.0), p + Vector2::new(-6.0, 0.0)]);
                    b.draw_colored_polygon(&pts, Color::from_rgb(0.85, 0.85, 0.95));
                }
                _ => {
                    b.draw_rect(Rect2::new(p - Vector2::new(6.0, 6.0), Vector2::new(12.0, 12.0)), Color::from_rgb(0.3, 0.55, 0.95));
                }
            }
        }

        // Enemies.
        let enemies: Vec<(Vector2, Vector2, String, f32, f32, i32)> =
            self.enemies.values().map(|e| (e.pos, e.facing, e.kind.clone(), e.hp, e.max_hp, e.level)).collect();
        for (p, facing, kind, hp, max_hp, level) in enemies {
            let (color, r) = enemy_look(&kind);
            {
                let mut b = self.base_mut();
                b.draw_circle(p + Vector2::new(0.0, r * 0.6), r * 0.9, Color::from_rgba(0.0, 0.0, 0.0, 0.25));
                b.draw_circle(p, r + 1.5, color.darkened(0.5));
                b.draw_circle(p, r, color);
                let f = if facing == Vector2::ZERO { Vector2::new(0.0, 1.0) } else { facing };
                let side = Vector2::new(-f.y, f.x) * (r * 0.35);
                let eye = p + f * (r * 0.45);
                let eye_c = if kind == "skeleton" { Color::from_rgb(0.9, 0.1, 0.1) } else { Color::BLACK };
                b.draw_circle(eye + side, r * 0.15 + 0.5, eye_c);
                b.draw_circle(eye - side, r * 0.15 + 0.5, eye_c);
            }
            self.text(p + Vector2::new(0.0, -r - 9.0), &format!("{kind} {level}"), 11, Color::from_rgb(1.0, 0.8, 0.75));
            if hp < max_hp {
                self.hp_bar(p + Vector2::new(0.0, -r - 6.0), 28.0, hp, max_hp, Color::from_rgb(0.9, 0.2, 0.2));
            }
        }

        // Players.
        let players: Vec<(i64, Vector2, Vector2, String, f32, f32, i32)> =
            self.players.iter().map(|(id, e)| (*id, e.pos, e.facing, e.name.clone(), e.hp, e.max_hp, e.level)).collect();
        for (id, p, facing, name, hp, max_hp, level) in players {
            let me = id == self.my_id;
            let body = if me { Color::from_rgb(0.98, 0.78, 0.3) } else { Color::from_rgb(0.4, 0.65, 1.0) };
            {
                let mut b = self.base_mut();
                b.draw_circle(p + Vector2::new(0.0, 8.0), 10.0, Color::from_rgba(0.0, 0.0, 0.0, 0.3));
                b.draw_circle(p, 13.0, body.darkened(0.55));
                b.draw_circle(p, 11.0, body);
                let f = if facing == Vector2::ZERO { Vector2::new(0.0, 1.0) } else { facing };
                b.draw_line(p + f * 8.0, p + f * 20.0, Color::from_rgb(0.85, 0.85, 0.92));
                b.draw_circle(p + f * 5.0, 3.0, body.darkened(0.6));
            }
            self.text(p + Vector2::new(0.0, -26.0), &format!("{name} [{level}]"), 13, if me { Color::from_rgb(1.0, 0.95, 0.7) } else { Color::WHITE });
            self.hp_bar(p + Vector2::new(0.0, -21.0), 32.0, hp, max_hp, Color::from_rgb(0.3, 0.9, 0.35));
        }

        // Attack swings.
        let swings = self.swings.clone();
        for (p, t) in swings {
            let k = t / 0.2;
            let a = (1.0 - k).clamp(0.0, 1.0);
            self.base_mut().draw_arc(p, 18.0 + 30.0 * k, 0.0, TAU, 32, Color::from_rgba(1.0, 1.0, 1.0, a * 0.6));
        }

        // Floating combat text.
        let floaters: Vec<(Vector2, String, Color, f32)> = self.floaters.iter().map(|f| (f.pos, f.text.clone(), f.color, f.t)).collect();
        for (p, text, mut color, t) in floaters {
            color.a = (1.2 - t).clamp(0.0, 1.0);
            let size = if text.starts_with("LEVEL") { 22 } else { 15 };
            let wobble = (t * PI * 4.0).sin() * 1.5;
            self.text(p + Vector2::new(wobble, 0.0), &text, size, color);
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('[', "[lb]")
}
