//! Players, enemies and ground items as 3D nodes, synced from server state
//! and smoothly interpolated between ticks.

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

use godot::classes::base_material_3d::BillboardMode;
use godot::classes::label_3d::DrawFlags;
use godot::classes::{Label3D, Material, MeshInstance3D, Node3D, QuadMesh, Shader, ShaderMaterial};
use godot::prelude::*;

use super::models::{self, ActorModel};
use crate::content::{parse_color, ContentLibrary};
use crate::net::protocol::{EnemyState, ItemState, PlayerState};
use crate::net::Catalog;

const HP_BAR_SHADER: &str = r#"
shader_type spatial;
render_mode unshaded, depth_test_disabled, cull_disabled, shadows_disabled;
instance uniform float fill = 1.0;
instance uniform vec4 bar_color : source_color = vec4(0.3, 0.9, 0.35, 1.0);
void vertex() {
    // Billboard: keep the quad facing the camera.
    MODELVIEW_MATRIX = VIEW_MATRIX * mat4(INV_VIEW_MATRIX[0], INV_VIEW_MATRIX[1], INV_VIEW_MATRIX[2], MODEL_MATRIX[3]);
}
void fragment() {
    vec3 c = UV.x < fill ? bar_color.rgb : vec3(0.12);
    if (UV.x < 0.03 || UV.x > 0.97 || UV.y < 0.18 || UV.y > 0.82) c = vec3(0.0);
    ALBEDO = c;
}
"#;

pub fn to_world(x: f32, y: f32) -> Vector3 {
    Vector3::new(x, 0.0, y)
}

struct Actor {
    model: ActorModel,
    label: Gd<Label3D>,
    bar: Gd<MeshInstance3D>,
    pos: Vector3,
    target: Vector3,
    yaw: f32,
    walk: f32,
    swing: f32,
    hp: f32,
    max_hp: f32,
    gear: (String, String),
    is_slime: bool,
    scale: f32,
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
            let fill = (hp / max_hp.max(1.0)).clamp(0.0, 1.0);
            self.bar
                .set_instance_shader_parameter("fill", &fill.to_variant());
        }
    }

    fn update(&mut self, delta: f32) {
        let a = 1.0 - (-delta * 14.0).exp();
        let before = self.pos;
        self.pos = self.pos.lerp(self.target, a);
        let moved = self.pos - before;
        let speed = moved.length() / delta.max(1e-4);
        if moved.length() > 0.002 {
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
            self.walk += delta * 12.0;
        } else {
            self.walk = 0.0;
        }
        let (bob, squash) = if self.is_slime {
            let s = (self.walk * 0.7).sin().abs();
            (s * 0.25, 1.0 - s * 0.2)
        } else {
            ((self.walk).sin().abs() * 0.06, 1.0)
        };
        let root = &mut self.model.root;
        root.set_position(self.pos + Vector3::new(0.0, bob, 0.0));
        root.set_rotation(Vector3::new(0.0, self.yaw, 0.0));
        if self.is_slime {
            let (s, w) = (self.scale, 1.0 / squash.sqrt());
            root.set_scale(Vector3::new(s * w, s * squash, s * w));
        }
        if let Some(p) = &mut self.model.weapon_pivot {
            if self.swing > 0.0 {
                self.swing = (self.swing - delta).max(0.0);
                let k = 1.0 - self.swing / 0.3;
                p.set_rotation(Vector3::new(
                    (k * PI).sin() * 1.4,
                    -(k * PI).sin() * 0.9,
                    0.0,
                ));
            } else {
                p.set_rotation(Vector3::new((self.walk).sin() * 0.3, 0.0, 0.0));
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
    items: HashMap<i64, GroundItem>,
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
        bar_mesh.set_size(Vector2::new(0.9, 0.11));
        Entities {
            root,
            bar_material,
            bar_mesh,
            players: HashMap::new(),
            enemies: HashMap::new(),
            items: HashMap::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_actor(
        &mut self,
        mut model: ActorModel,
        scale: f32,
        name: &str,
        color: Color,
        bar_color: Color,
        x: f32,
        y: f32,
        slime: bool,
    ) -> Actor {
        model.root.set_scale(Vector3::splat(scale));
        let h = model.height * scale;

        let mut label = Label3D::new_alloc();
        label.set_text(name);
        label.set_billboard_mode(BillboardMode::ENABLED);
        label.set_draw_flag(DrawFlags::DISABLE_DEPTH_TEST, true);
        label.set_font_size(40);
        label.set_outline_size(10);
        label.set_pixel_size(0.006);
        label.set_modulate(color);
        label.set_render_priority(2);
        label.set_position(Vector3::new(0.0, h + 0.45, 0.0) / scale);
        model.root.add_child(&label);

        let mut bar = MeshInstance3D::new_alloc();
        bar.set_mesh(&self.bar_mesh);
        bar.set_material_override(&self.bar_material.clone().upcast::<Material>());
        bar.set_position(Vector3::new(0.0, h + 0.25, 0.0) / scale);
        bar.set_scale(Vector3::splat(1.0 / scale));
        bar.set_instance_shader_parameter("bar_color", &bar_color.to_variant());
        model.root.add_child(&bar);

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
            gear: ("?".into(), "?".into()), // forces the first gear update
            is_slime: slime,
            scale,
        }
    }

    pub fn sync_players(
        &mut self,
        list: &[PlayerState],
        my_id: i64,
        catalog: &Catalog,
        lib: &ContentLibrary,
    ) {
        let mut seen = HashSet::new();
        for p in list {
            seen.insert(p.id);
            if !self.players.contains_key(&p.id) {
                let me = p.id == my_id;
                let shirt = if me {
                    Color::from_rgb(0.95, 0.75, 0.3)
                } else {
                    Color::from_rgb(0.35, 0.55, 0.95)
                };
                let model = models::actor("player", lib, "cloth", shirt);
                let label_color = if me {
                    Color::from_rgb(1.0, 0.92, 0.6)
                } else {
                    Color::WHITE
                };
                let a = self.spawn_actor(
                    model,
                    1.0,
                    "",
                    label_color,
                    Color::from_rgb(0.3, 0.9, 0.35),
                    p.x,
                    p.y,
                    false,
                );
                self.players.insert(p.id, a);
            }
            let a = self.players.get_mut(&p.id).unwrap();
            a.set_target(p.x, p.y);
            a.set_hp(p.hp, p.max_hp);
            a.label.set_text(&format!("{} [{}]", p.name, p.level));
            if a.gear != (p.weapon.clone(), p.armor.clone()) {
                a.gear = (p.weapon.clone(), p.armor.clone());
                if let Some(w) = &mut a.model.weapon {
                    w.set_visible(!p.weapon.is_empty());
                    if let Some(def) = catalog.item(&p.weapon) {
                        w.set_material_override(
                            &lib.material("metal", parse_color(&def.color), 1.0, false)
                                .upcast::<Material>(),
                        );
                    }
                }
                if let (Some(body), Some(def)) = (&mut a.model.body, catalog.item(&p.armor)) {
                    body.set_material_override(
                        &lib.material("metal", parse_color(&def.color), 1.0, false)
                            .upcast::<Material>(),
                    );
                } else if let Some(body) = &mut a.model.body {
                    let shirt = if p.id == my_id {
                        Color::from_rgb(0.95, 0.75, 0.3)
                    } else {
                        Color::from_rgb(0.35, 0.55, 0.95)
                    };
                    body.set_material_override(
                        &lib.material("cloth", shirt, 1.0, false)
                            .upcast::<Material>(),
                    );
                }
            }
        }
        self.players.retain(|id, a| {
            let keep = seen.contains(id);
            if !keep {
                a.free();
            }
            keep
        });
    }

    pub fn sync_enemies(&mut self, list: &[EnemyState], catalog: &Catalog, lib: &ContentLibrary) {
        let mut seen = HashSet::new();
        for e in list {
            seen.insert(e.id);
            if !self.enemies.contains_key(&e.id) {
                let def = catalog.enemy(&e.kind).cloned().unwrap_or_default();
                let color = parse_color(&def.color);
                let model = models::actor(&def.model, lib, &def.texture, color);
                let name = format!("{} {}", def.name, e.level);
                let scale = if def.scale > 0.0 { def.scale } else { 1.0 };
                let a = self.spawn_actor(
                    model,
                    scale,
                    &name,
                    Color::from_rgb(1.0, 0.75, 0.7),
                    Color::from_rgb(0.9, 0.25, 0.2),
                    e.x,
                    e.y,
                    def.model == "slime",
                );
                self.enemies.insert(e.id, a);
            }
            let a = self.enemies.get_mut(&e.id).unwrap();
            a.set_target(e.x, e.y);
            a.set_hp(e.hp, e.max_hp);
            a.bar.set_visible(e.hp < e.max_hp);
        }
        self.enemies.retain(|id, a| {
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
                let (kind, color) = match catalog.item(&it.item) {
                    Some(d) => (d.kind.as_str(), parse_color(&d.color)),
                    None => ("gold", Color::from_rgb(1.0, 0.82, 0.2)),
                };
                let node = models::ground_item(lib, kind, color);
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

    pub fn update(&mut self, delta: f32) {
        for a in self.players.values_mut().chain(self.enemies.values_mut()) {
            a.update(delta);
        }
        for it in self.items.values_mut() {
            it.t += delta;
            it.node
                .set_position(it.pos + Vector3::new(0.0, 0.35 + (it.t * 3.0).sin() * 0.08, 0.0));
            it.node.set_rotation(Vector3::new(0.0, it.t * 1.8, 0.0));
        }
    }

    pub fn clear(&mut self) {
        for a in self.players.values_mut().chain(self.enemies.values_mut()) {
            a.free();
        }
        self.players.clear();
        self.enemies.clear();
        for it in self.items.values_mut() {
            it.node.queue_free();
        }
        self.items.clear();
    }
}
