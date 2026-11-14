//! Players, enemies, NPCs, pets and ground items as 3D nodes, synced from
//! server state and smoothly interpolated between ticks.

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

use godot::classes::base_material_3d::{BillboardMode, ShadingMode, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::label_3d::DrawFlags;
use godot::classes::{
    Camera3D, Label3D, Material, MeshInstance3D, Node3D, QuadMesh, Shader, ShaderMaterial,
    StandardMaterial3D,
};
use godot::prelude::*;

use super::models::{self, ActorModel, HumanStyle};
use crate::content::{parse_color, ContentLibrary};
use crate::net::protocol::{EnemyState, ItemState, Look, NpcInfo, PetState, PlayerState};
use crate::net::Catalog;

const HP_BAR_SHADER: &str = r#"
shader_type spatial;
render_mode unshaded, depth_test_disabled, cull_disabled, shadows_disabled;
instance uniform float fill = 1.0;
instance uniform vec4 bar_color : source_color = vec4(0.3, 0.9, 0.35, 1.0);
void vertex() {
    MODELVIEW_MATRIX = VIEW_MATRIX * mat4(INV_VIEW_MATRIX[0], INV_VIEW_MATRIX[1], INV_VIEW_MATRIX[2], MODEL_MATRIX[3]);
}
void fragment() {
    vec3 c = UV.x < fill ? bar_color.rgb * (0.75 + 0.35 * (1.0 - UV.y)) : vec3(0.1);
    if (UV.x < 0.02 || UV.x > 0.98 || UV.y < 0.15 || UV.y > 0.85) c = vec3(0.0);
    ALBEDO = c;
}
"#;

pub const RANK_COLORS: [&str; 6] = [
    "#b0b0b0", "#ffd8d0", "#9ad0ff", "#ffd24a", "#ff8a2a", "#ff3a3a",
];

pub fn to_world(x: f32, y: f32) -> Vector3 {
    Vector3::new(x, super::terrain::height(x, y), y)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Enemy(i64),
    Npc(i64),
    Player(i64),
}

struct Actor {
    model: ActorModel,
    label: Gd<Label3D>,
    bar: Option<Gd<MeshInstance3D>>,
    pos: Vector3,
    target: Vector3,
    yaw: f32,
    walk: f32,
    swing: f32,
    hp: f32,
    max_hp: f32,
    sig: String,
    slime: bool,
    scale: f32,
    t: f32,
    fixed_yaw: Option<f32>,
    frozen: bool,
}

impl Actor {
    fn free(&mut self) {
        self.model.root.queue_free();
    }

    fn set_target(&mut self, x: f32, y: f32) {
        self.target = to_world(x, y);
    }

    fn set_hp(&mut self, hp: f32, max_hp: f32) {
        if (hp, max_hp) != (self.hp, self.max_hp) {
            self.hp = hp;
            self.max_hp = max_hp;
            if let Some(bar) = &mut self.bar {
                let fill = (hp / max_hp.max(1.0)).clamp(0.0, 1.0);
                bar.set_instance_shader_parameter("fill", &fill.to_variant());
            }
        }
    }

    fn head_pos(&self) -> Vector3 {
        self.pos + Vector3::new(0.0, self.model.height * self.scale + self.model.float, 0.0)
    }

    fn update(&mut self, delta: f32) {
        let delta = if self.frozen { delta * 0.05 } else { delta };
        self.t += delta;
        let a = 1.0 - (-delta * 14.0).exp();
        let before = self.pos;
        if self.pos.distance_to(self.target) > 20.0 {
            self.pos = self.target;
        }
        self.pos = self.pos.lerp(self.target, a);
        self.pos.y = super::terrain::height(self.pos.x, self.pos.z);
        let moved = self.pos - before;
        let speed = Vector2::new(moved.x, moved.z).length() / delta.max(1e-4);
        if let Some(y) = self.fixed_yaw {
            self.yaw = y;
        } else if Vector2::new(moved.x, moved.z).length() > 0.002 {
            let want = moved.x.atan2(moved.z);
            let mut d = want - self.yaw;
            while d > PI {
                d -= 2.0 * PI;
            }
            while d < -PI {
                d += 2.0 * PI;
            }
            self.yaw += d * (1.0 - (-delta * 12.0).exp());
        }
        if speed > 0.3 {
            self.walk += delta * (6.0 + speed * 1.6);
        } else {
            self.walk *= 1.0 - (delta * 8.0).min(1.0);
        }
        let stride = self.walk.sin();
        let (bob, squash) = if self.slime {
            let s = (self.walk * 0.7).sin().abs();
            (s * 0.25, 1.0 - s * 0.2)
        } else {
            ((self.walk * 2.0).sin().abs() * 0.04, 1.0)
        };
        let m = &mut self.model;
        let quad = m.legs.len() >= 4;
        for (i, leg) in m.legs.iter_mut().enumerate() {
            let phase = if quad {
                if i == 0 || i == 3 {
                    1.0
                } else {
                    -1.0
                }
            } else if i % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            leg.set_rotation(Vector3::new(stride * 0.5 * phase, 0.0, 0.0));
        }
        for (i, arm) in m.arms.iter_mut().enumerate() {
            let phase = if i == 0 { -1.0 } else { 1.0 };
            let idle = (self.t * 1.6).sin() * 0.03;
            arm.set_rotation(Vector3::new(stride * 0.45 * phase + idle, 0.0, 0.0));
        }
        if let Some(h) = &mut m.head {
            h.set_rotation(Vector3::new(
                0.0,
                (self.t * 0.5).sin() * 0.15 * (speed < 0.3) as i32 as f32,
                0.0,
            ));
        }
        for (i, s) in m.spin.iter_mut().enumerate() {
            let r = s.get_rotation();
            let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
            s.set_rotation(Vector3::new(r.x, r.y + delta * 22.0 * dir, r.z));
        }
        let flap = (self.t * if m.float > 0.0 { 14.0 } else { 3.0 }).sin();
        for (i, w) in m.wings.iter_mut().enumerate() {
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            w.set_rotation(Vector3::new(0.0, 0.0, flap * 0.6 * side));
        }
        let hover = if m.float > 0.0 {
            m.float + (self.t * 2.3).sin() * 0.12
        } else {
            0.0
        };
        let root = &mut m.root;
        root.set_position(self.pos + Vector3::new(0.0, bob + hover, 0.0));
        root.set_rotation(Vector3::new(0.0, self.yaw, 0.0));
        if self.slime {
            let (s, w) = (self.scale, 1.0 / squash.sqrt());
            root.set_scale(Vector3::new(s * w, s * squash, s * w));
        }
        if let Some(p) = &mut m.weapon_pivot {
            if self.swing > 0.0 {
                self.swing = (self.swing - delta).max(0.0);
                let k = 1.0 - self.swing / 0.3;
                p.set_rotation(Vector3::new(
                    -(k * PI).sin() * 1.6,
                    -(k * PI).sin() * 0.7,
                    0.0,
                ));
            } else {
                p.set_rotation(Vector3::new(0.4, 0.0, 0.0));
            }
        }
    }
}

struct GroundItem {
    node: Gd<MeshInstance3D>,
    pos: Vector3,
    t: f32,
}

pub struct Entities {
    root: Gd<Node3D>,
    bar_material: Gd<ShaderMaterial>,
    bar_mesh: Gd<QuadMesh>,
    players: HashMap<i64, Actor>,
    enemies: HashMap<i64, Actor>,
    npcs: HashMap<i64, Actor>,
    pets: HashMap<i64, Actor>,
    items: HashMap<i64, GroundItem>,
    ring: Gd<MeshInstance3D>,
    ring_mat: Gd<StandardMaterial3D>,
    pub target: i64,
}

fn style_from_look(look: &Look) -> HumanStyle {
    HumanStyle {
        look: look.clone(),
        ..Default::default()
    }
}

fn outfit_style(info: &NpcInfo) -> HumanStyle {
    let mut st = style_from_look(&info.look);
    let c = parse_color(&info.color);
    st.outfit = info.outfit.clone();
    st.body = c;
    st.legs = c.darkened(0.4);
    st.accent = c.lightened(0.3);
    match info.outfit.as_str() {
        "guard" => {
            st.body_tex = "steel".into();
            st.body = Color::from_rgb(0.62, 0.64, 0.7);
            st.accent = Color::from_rgb(0.75, 0.2, 0.15);
            st.helm = Some(Color::from_rgb(0.7, 0.72, 0.78));
            st.weapon = "spear".into();
            st.offhand = "shield".into();
            st.offhand_color = Color::from_rgb(0.7, 0.2, 0.15);
        }
        "smith" => {
            st.body_tex = "leather".into();
            st.weapon = "hammer".into();
        }
        "robe" => {
            st.weapon = "staff".into();
            st.weapon_color = Color::from_rgb(0.5, 0.6, 1.0);
        }
        "priest" => {
            st.body = Color::from_rgb(0.92, 0.9, 0.84);
            st.accent = Color::from_rgb(0.8, 0.65, 0.25);
        }
        "royal" => {
            st.cape = Some(Color::from_rgb(0.6, 0.08, 0.12));
            st.accent = Color::from_rgb(1.0, 0.8, 0.3);
            st.weapon = "sword".into();
            st.weapon_color = Color::from_rgb(1.0, 0.85, 0.4);
        }
        "cloak" => {
            st.accent = c.darkened(0.2);
            st.cape = Some(c.darkened(0.3));
        }
        "leather" => st.body_tex = "leather".into(),
        "dress" => {
            st.look.female = true;
            st.look.hair = 2;
        }
        _ => {}
    }
    st
}

fn player_style(p: &PlayerState, me: bool) -> HumanStyle {
    let mut st = style_from_look(&p.look);
    st.outfit = "adventurer".into();
    st.body = if p.armor.is_empty() {
        if me {
            Color::from_rgb(0.9, 0.7, 0.35)
        } else {
            Color::from_rgb(0.35, 0.5, 0.85)
        }
    } else {
        parse_color(&p.armor)
    };
    if !p.armor.is_empty() {
        st.body_tex = "steel".into();
    }
    st.legs = Color::from_rgb(0.32, 0.26, 0.2);
    st.weapon = p.weapon.clone();
    st.weapon_color = if p.wcolor.is_empty() {
        Color::from_rgb(0.85, 0.86, 0.9)
    } else {
        parse_color(&p.wcolor)
    };
    st.helm = (!p.helm.is_empty()).then(|| parse_color(&p.helm));
    if let Some((kind, color)) = p.shield.split_once(':') {
        st.offhand = kind.into();
        st.offhand_color = parse_color(color);
    }
    st.cape = Some(if p.title.is_empty() {
        Color::from_rgb(0.55, 0.15, 0.12)
    } else {
        Color::from_rgb(0.2, 0.25, 0.6)
    });
    st
}

impl Entities {
    pub fn new(parent: &mut Gd<Node3D>) -> Entities {
        let mut root = Node3D::new_alloc();
        root.set_name("Entities");
        parent.add_child(&root);
        let mut shader = Shader::new_gd();
        shader.set_code(HP_BAR_SHADER);
        let mut bar_material = ShaderMaterial::new_gd();
        bar_material.set_shader(&shader);
        let mut bar_mesh = QuadMesh::new_gd();
        bar_mesh.set_size(Vector2::new(0.9, 0.1));
        let mut ring_mat = StandardMaterial3D::new_gd();
        ring_mat.set_shading_mode(ShadingMode::UNSHADED);
        ring_mat.set_transparency(Transparency::ALPHA);
        ring_mat.set_albedo(Color::from_rgba(1.0, 0.25, 0.2, 0.85));
        let mut ring = models::instance(
            &models::torus(0.62, 0.7),
            &ring_mat,
            models::trs(Vector3::ZERO, 0.0, Vector3::new(1.0, 0.15, 1.0)),
        );
        ring.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        ring.set_visible(false);
        root.add_child(&ring);
        Entities {
            root,
            bar_material,
            bar_mesh,
            players: HashMap::new(),
            enemies: HashMap::new(),
            npcs: HashMap::new(),
            pets: HashMap::new(),
            items: HashMap::new(),
            ring,
            ring_mat,
            target: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_actor(
        &mut self,
        mut model: ActorModel,
        scale: f32,
        name: &str,
        color: Color,
        bar_color: Option<Color>,
        x: f32,
        y: f32,
        slime: bool,
    ) -> Actor {
        model.root.set_scale(Vector3::splat(scale));
        let h = model.height * scale + model.float;
        let mut label = Label3D::new_alloc();
        label.set_text(name);
        label.set_billboard_mode(BillboardMode::ENABLED);
        label.set_draw_flag(DrawFlags::DISABLE_DEPTH_TEST, true);
        label.set_font_size(30);
        label.set_outline_size(9);
        label.set_draw_flag(DrawFlags::FIXED_SIZE, true);
        label.set_pixel_size(0.0011);
        label.set_modulate(color);
        label.set_render_priority(2);
        label.set_position(Vector3::new(0.0, h + 0.42, 0.0) / scale);
        label.set_visibility_range_end(32.0);
        label.set_scale(Vector3::splat(1.0 / scale));
        model.root.add_child(&label);

        let bar = bar_color.map(|bc| {
            let mut bar = MeshInstance3D::new_alloc();
            bar.set_mesh(&self.bar_mesh);
            bar.set_material_override(&self.bar_material.clone().upcast::<Material>());
            bar.set_position(Vector3::new(0.0, h + 0.22, 0.0) / scale);
            bar.set_scale(Vector3::splat(1.0 / scale));
            bar.set_cast_shadows_setting(ShadowCastingSetting::OFF);
            bar.set_instance_shader_parameter("bar_color", &bc.to_variant());
            model.root.add_child(&bar);
            bar
        });

        self.root.add_child(&model.root);
        let pos = to_world(x, y);
        Actor {
            model,
            label,
            bar,
            pos,
            target: pos,
            yaw: 0.0,
            walk: 0.0,
            swing: 0.0,
            hp: -1.0,
            max_hp: -1.0,
            sig: String::new(),
            slime,
            scale,
            t: (x * 7.0 + y * 3.0) % 10.0,
            fixed_yaw: None,
            frozen: false,
        }
    }

    pub fn sync_players(
        &mut self,
        list: &[PlayerState],
        my_id: i64,
        _catalog: &Catalog,
        lib: &ContentLibrary,
    ) {
        let mut seen = HashSet::new();
        for p in list {
            seen.insert(p.id);
            let me = p.id == my_id;
            let sig = format!(
                "{:?}{}{}{}{}{}{}",
                p.look, p.weapon, p.wcolor, p.armor, p.helm, p.shield, p.title
            );
            if self.players.get(&p.id).is_some_and(|a| a.sig != sig) {
                if let Some(mut a) = self.players.remove(&p.id) {
                    a.free();
                }
            }
            if !self.players.contains_key(&p.id) {
                let model = models::humanoid(lib, &player_style(p, me));
                let label_color = if me {
                    Color::from_rgb(1.0, 0.92, 0.6)
                } else {
                    Color::from_rgb(0.75, 0.9, 1.0)
                };
                let mut a = self.spawn_actor(
                    model,
                    1.0,
                    "",
                    label_color,
                    Some(Color::from_rgb(0.3, 0.9, 0.35)),
                    p.x,
                    p.y,
                    false,
                );
                a.sig = sig;
                self.players.insert(p.id, a);
            }
            let a = self.players.get_mut(&p.id).unwrap();
            a.set_target(p.x, p.y);
            a.set_hp(p.hp, p.max_hp);
            let title = if p.title.is_empty() {
                String::new()
            } else {
                format!("\n<{}>", p.title)
            };
            a.label
                .set_text(&format!("{} [{}]{}", p.name, p.level, title));
        }
        self.players.retain(|id, a| {
            let keep = seen.contains(id);
            if !keep {
                a.free();
            }
            keep
        });
    }

    pub fn sync_enemies(&mut self, list: &[EnemyState], lib: &ContentLibrary) {
        let mut seen = HashSet::new();
        for e in list {
            seen.insert(e.id);
            if !self.enemies.contains_key(&e.id) {
                let color = parse_color(&e.color);
                let mut model = models::actor(&e.model, lib, &e.tex, color);
                if !e.glow.is_empty() {
                    let glow = parse_color(&e.glow);
                    let mut l = models::light(glow, if e.rank >= 3 { 1.6 } else { 0.8 }, 3.5);
                    l.set_position(Vector3::new(0.0, 1.0, 0.0));
                    model.root.add_child(&l);
                    if let Some(body) = &mut model.body {
                        if let Some(mat) = body
                            .get_material_override()
                            .and_then(|m| m.try_cast::<StandardMaterial3D>().ok())
                        {
                            let mut mat = mat.duplicate_resource();
                            mat.set_feature(
                                godot::classes::base_material_3d::Feature::EMISSION,
                                true,
                            );
                            mat.set_emission(glow);
                            mat.set_emission_energy_multiplier(0.35);
                            body.set_material_override(&mat.upcast::<Material>());
                        }
                    }
                }
                if e.rank >= 3 {
                    let rc = parse_color(RANK_COLORS[e.rank.clamp(0, 5) as usize]);
                    let mut aura = models::instance(
                        &models::torus(0.7, 0.78),
                        &models::ghost_material(Color::from_rgba(rc.r, rc.g, rc.b, 0.6)),
                        models::trs(
                            Vector3::new(0.0, 0.05, 0.0),
                            0.0,
                            Vector3::new(1.0, 0.1, 1.0),
                        ),
                    );
                    aura.set_cast_shadows_setting(ShadowCastingSetting::OFF);
                    model.root.add_child(&aura);
                }
                let name = format!("{} {}", e.name, e.level);
                let scale = if e.scale > 0.0 { e.scale } else { 1.0 };
                let rank_color = parse_color(RANK_COLORS[e.rank.clamp(0, 5) as usize]);
                let a = self.spawn_actor(
                    model,
                    scale,
                    &name,
                    rank_color,
                    Some(Color::from_rgb(0.9, 0.25, 0.2)),
                    e.x,
                    e.y,
                    e.model == "slime",
                );
                self.enemies.insert(e.id, a);
            }
            let a = self.enemies.get_mut(&e.id).unwrap();
            a.set_target(e.x, e.y);
            a.set_hp(e.hp, e.max_hp);
            if let Some(bar) = &mut a.bar {
                bar.set_visible(e.hp < e.max_hp || e.id == self.target);
            }
            a.frozen = e.status.iter().any(|s| s == "freeze" || s == "stun");
        }
        self.enemies.retain(|id, a| {
            let keep = seen.contains(id);
            if !keep {
                a.free();
            }
            keep
        });
    }

    pub fn spawn_npcs(&mut self, list: &[NpcInfo], lib: &ContentLibrary) {
        for n in list {
            let model = if n.role == "board" {
                notice_board(lib)
            } else {
                models::humanoid(lib, &outfit_style(n))
            };
            let label = format!("{}\n<{}>", n.name, n.title);
            let mut a = self.spawn_actor(
                model,
                1.0,
                &label,
                Color::from_rgb(0.6, 1.0, 0.6),
                None,
                n.x,
                n.y,
                false,
            );
            a.fixed_yaw = Some(n.yaw);
            a.label.set_font_size(26);
            a.label.set_visibility_range_end(18.0);
            if !n.marker.is_empty() {
                let mut mk = Label3D::new_alloc();
                mk.set_text(&n.marker);
                mk.set_billboard_mode(BillboardMode::ENABLED);
                mk.set_draw_flag(DrawFlags::DISABLE_DEPTH_TEST, true);
                mk.set_font_size(54);
                mk.set_outline_size(12);
                mk.set_draw_flag(DrawFlags::FIXED_SIZE, true);
                mk.set_pixel_size(0.0014);
                mk.set_modulate(Color::from_rgb(1.0, 0.85, 0.2));
                mk.set_position(Vector3::new(0.0, 2.75, 0.0));
                mk.set_visibility_range_end(45.0);
                a.model.root.add_child(&mk);
            }
            self.npcs.insert(n.id, a);
        }
    }

    pub fn sync_pets(&mut self, list: &[PetState], lib: &ContentLibrary) {
        let mut seen = HashSet::new();
        for p in list {
            seen.insert(p.id);
            if !self.pets.contains_key(&p.id) {
                let model = models::actor(&p.model, lib, &p.tex, parse_color(&p.color));
                let scale = if p.scale > 0.0 { p.scale } else { 0.7 };
                let a = self.spawn_actor(
                    model,
                    scale,
                    "",
                    Color::from_rgb(0.85, 0.95, 0.7),
                    Some(Color::from_rgb(0.35, 0.75, 1.0)),
                    p.x,
                    p.y,
                    p.model == "slime",
                );
                self.pets.insert(p.id, a);
            }
            let a = self.pets.get_mut(&p.id).unwrap();
            a.set_target(p.x, p.y);
            a.set_hp(p.hp, p.max_hp);
            a.label.set_text(&format!("{} [{}]", p.name, p.level));
        }
        self.pets.retain(|id, a| {
            let keep = seen.contains(id);
            if !keep {
                a.free();
            }
            keep
        });
    }

    pub fn sync_items(&mut self, list: &[ItemState], catalog: &Catalog, lib: &ContentLibrary) {
        let mut seen = HashSet::new();
        for it in list {
            seen.insert(it.id);
            if !self.items.contains_key(&it.id) {
                let (kind, color) = if it.item.starts_with("gear:") {
                    ("gear", parse_color(&it.color))
                } else {
                    match catalog.item(&it.item) {
                        Some(d) => (d.kind.as_str(), parse_color(&d.color)),
                        None => ("gold", Color::from_rgb(1.0, 0.82, 0.2)),
                    }
                };
                let mut node = models::ground_item(lib, kind, color);
                if kind == "gear" && it.color != "#d8d8d8" {
                    let mut beam = models::instance(
                        &models::cylinder(0.04, 0.12, 3.0, 10),
                        &models::ghost_material(Color::from_rgba(color.r, color.g, color.b, 0.35)),
                        models::at(Vector3::new(0.0, 1.4, 0.0)),
                    );
                    beam.set_cast_shadows_setting(ShadowCastingSetting::OFF);
                    node.add_child(&beam);
                }
                self.root.add_child(&node);
                self.items.insert(
                    it.id,
                    GroundItem {
                        node,
                        pos: to_world(it.x, it.y),
                        t: (it.id % 100) as f32 * 0.1,
                    },
                );
            }
        }
        self.items.retain(|id, it| {
            let keep = seen.contains(id);
            if !keep {
                it.node.queue_free();
            }
            keep
        });
    }

    /// Plays the weapon swing on the player standing at (x, y).
    pub fn swing_at(&mut self, x: f32, y: f32) {
        let p = to_world(x, y);
        if let Some(a) = self
            .players
            .values_mut()
            .min_by(|a, b| a.target.distance_to(p).total_cmp(&b.target.distance_to(p)))
        {
            if a.target.distance_to(p) < 0.5 {
                a.swing = 0.3;
            }
        }
    }

    pub fn player_position(&self, id: i64) -> Option<Vector3> {
        self.players.get(&id).map(|a| a.pos)
    }

    /// Finds the entity closest to a screen point (within 48 px of its body).
    pub fn pick(&self, cam: &Gd<Camera3D>, screen: Vector2) -> Option<Pick> {
        let mut best: Option<(f32, Pick)> = None;
        let mut consider = |a: &Actor, p: Pick| {
            let mid =
                a.pos + Vector3::new(0.0, (a.model.height * a.scale) * 0.55 + a.model.float, 0.0);
            if cam.is_position_behind(mid) || cam.get_global_position().distance_to(mid) > 70.0 {
                return;
            }
            let sp = cam.unproject_position(mid);
            let radius = 48.0 * a.scale.max(0.8);
            let d = sp.distance_to(screen);
            if d < radius && best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, p));
            }
        };
        for (id, a) in &self.enemies {
            consider(a, Pick::Enemy(*id));
        }
        for (id, a) in &self.npcs {
            consider(a, Pick::Npc(*id));
        }
        for (id, a) in &self.players {
            consider(a, Pick::Player(*id));
        }
        best.map(|b| b.1)
    }

    pub fn update(&mut self, delta: f32) {
        for a in self
            .players
            .values_mut()
            .chain(self.enemies.values_mut())
            .chain(self.npcs.values_mut())
            .chain(self.pets.values_mut())
        {
            a.update(delta);
        }
        for it in self.items.values_mut() {
            it.t += delta;
            it.node
                .set_position(it.pos + Vector3::new(0.0, 0.35 + (it.t * 3.0).sin() * 0.08, 0.0));
            it.node.set_rotation(Vector3::new(0.0, it.t * 1.8, 0.0));
        }
        match self.enemies.get(&self.target) {
            Some(a) => {
                self.ring.set_visible(true);
                let s = a.scale.max(0.7);
                self.ring.set_position(a.pos + Vector3::new(0.0, 0.06, 0.0));
                self.ring.set_rotation(Vector3::new(0.0, a.t * 1.5, 0.0));
                self.ring.set_scale(Vector3::new(s, 0.15, s));
                let pulse = 0.65 + (a.t * 5.0).sin() * 0.2;
                self.ring_mat
                    .set_albedo(Color::from_rgba(1.0, 0.25, 0.2, pulse));
                let _ = a.head_pos();
            }
            None => self.ring.set_visible(false),
        }
    }

    pub fn clear(&mut self) {
        for a in self
            .players
            .values_mut()
            .chain(self.enemies.values_mut())
            .chain(self.npcs.values_mut())
            .chain(self.pets.values_mut())
        {
            a.free();
        }
        self.players.clear();
        self.enemies.clear();
        self.npcs.clear();
        self.pets.clear();
        for it in self.items.values_mut() {
            it.node.queue_free();
        }
        self.items.clear();
    }
}

/// The tavern contract board: two posts, a plank and pinned notices.
fn notice_board(lib: &ContentLibrary) -> ActorModel {
    let wood = lib.material("wood", Color::from_rgb(0.8, 0.65, 0.5), 1.0, false);
    let paper = lib.material("cloth", Color::from_rgb(0.95, 0.9, 0.75), 1.0, false);
    let red = lib.material("cloth", Color::from_rgb(0.8, 0.2, 0.15), 1.0, false);
    let mut root = Node3D::new_alloc();
    for x in [-0.6, 0.6] {
        root.add_child(&models::instance(
            &models::cube(Vector3::new(0.1, 1.9, 0.1)),
            &wood,
            models::at(Vector3::new(x, 0.95, 0.0)),
        ));
    }
    root.add_child(&models::instance(
        &models::cube(Vector3::new(1.4, 0.9, 0.06)),
        &wood,
        models::at(Vector3::new(0.0, 1.35, 0.0)),
    ));
    root.add_child(&models::instance(
        &models::cube(Vector3::new(1.6, 0.12, 0.3)),
        &wood,
        models::at(Vector3::new(0.0, 1.9, 0.0)),
    ));
    for (i, (x, y)) in [
        (-0.4, 1.5),
        (0.0, 1.45),
        (0.4, 1.55),
        (-0.25, 1.15),
        (0.3, 1.18),
    ]
    .iter()
    .enumerate()
    {
        let mat = if i == 2 { &red } else { &paper };
        root.add_child(&models::instance(
            &models::cube(Vector3::new(0.28, 0.32, 0.01)),
            mat,
            models::rot(
                Vector3::new(*x, *y, 0.04),
                Vector3::new(0.0, 0.0, (i as f32 - 2.0) * 0.08),
                Vector3::ONE,
            ),
        ));
    }
    ActorModel {
        root,
        body: None,
        weapon_pivot: None,
        height: 2.0,
        legs: Vec::new(),
        arms: Vec::new(),
        spin: Vec::new(),
        wings: Vec::new(),
        float: 0.0,
        head: None,
    }
}
