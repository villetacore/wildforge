//! The 3D world view: everything drawn in the scene for one game session.
//! It owns Godot nodes under a single root and is rebuilt on every login.

pub mod anim;
pub mod camera;
pub mod effects;
pub mod entities;
pub mod environment;
pub mod grade;
pub mod models;
pub mod resources;
pub mod structures;
pub mod terrain;
pub mod town;

use godot::prelude::*;

use crate::content::{parse_color, ContentLibrary};
use crate::net::protocol::{Fx, PortalLink};
use crate::net::Catalog;
use crate::session::Session;
use camera::CameraRig;
use effects::Effects;
use entities::{to_world, Entities};
use resources::ResourceField;
use structures::DungeonView;
use terrain::Terrain;
use town::Town;

pub struct WorldView {
    root: Gd<Node3D>,
    terrain: Terrain,
    lighting: environment::Lighting,
    resources: ResourceField,
    town: Town,
    dungeons: Vec<DungeonView>,
    pub entities: Entities,
    effects: Effects,
    pub camera: CameraRig,
    pub grade: grade::Grade,
}

impl WorldView {
    pub fn new(
        parent: &mut Gd<Node3D>,
        session: &Session,
        depleted: &[usize],
        lib: &ContentLibrary,
    ) -> WorldView {
        let mut root = Node3D::new_alloc();
        root.set_name("World");
        parent.add_child(&root);
        let lighting = environment::build(&mut root);
        terrain::set_heights(session.size, &session.heights);
        let terrain = Terrain::build(
            &mut root,
            session.size,
            &session.terrain,
            &session.dungeons,
            lib,
        );
        let resources = ResourceField::build(
            &mut root,
            session.size,
            &session.nodes,
            depleted,
            &session.catalog,
            lib,
        );
        let town = Town::build(
            &mut root,
            &session.structures,
            session.size,
            &session.terrain,
            lib,
        );
        structures::build_surface_portals(&mut root, &session.dungeons, lib);
        let mut dungeons: Vec<DungeonView> = session
            .dungeons
            .iter()
            .cloned()
            .map(DungeonView::new)
            .collect();
        for d in &mut dungeons {
            d.rebuild(
                &mut root,
                session.size,
                &session.terrain,
                &session.portals,
                lib,
            );
        }
        let mut entities = Entities::new(&mut root);
        entities.spawn_npcs(&session.npcs, lib);
        let effects = Effects::new(&mut root);
        let camera = CameraRig::new(&mut root);
        let grade = grade::Grade::build(&mut root);
        WorldView {
            root,
            terrain,
            lighting,
            resources,
            town,
            dungeons,
            entities,
            effects,
            camera,
            grade,
        }
    }

    pub fn set_node_alive(&mut self, tile: usize, alive: bool) {
        self.resources.set_alive(tile, alive);
    }

    pub fn rebuild_dungeon(
        &mut self,
        index: usize,
        size: usize,
        codes: &[u8],
        portals: &[PortalLink],
        lib: &ContentLibrary,
    ) {
        let mut root = self.root.clone();
        if let Some(d) = self.dungeons.iter_mut().find(|d| d.info.index == index) {
            d.rebuild(&mut root, size, codes, portals, lib);
        }
    }

    pub fn play_fx(&mut self, fx: &[Fx], _catalog: &Catalog) {
        for f in fx {
            let pos = to_world(f.x, f.y);
            let custom = (!f.color.is_empty()).then(|| parse_color(&f.color));
            let (color, big) = match f.kind.as_str() {
                "swing" => {
                    self.entities.swing_at(f.x, f.y);
                    self.effects.ring(pos, Color::from_rgba(1.0, 1.0, 1.0, 0.5));
                    continue;
                }
                "bolt" => {
                    self.effects.bolt(
                        pos,
                        to_world(f.x2, f.y2),
                        custom.unwrap_or(Color::from_rgb(1.0, 0.6, 0.2)),
                    );
                    continue;
                }
                "nova" => {
                    self.effects
                        .nova(pos, f.r, custom.unwrap_or(Color::from_rgb(1.0, 0.8, 0.4)));
                    continue;
                }
                "shake" => {
                    self.camera.shake(f.r);
                    continue;
                }
                "cast" => {
                    self.entities.swing_at(f.x, f.y);
                    continue;
                }
                "gather" => {
                    self.resources.shake_at(f.x, f.y);
                    (Color::from_rgb(0.75, 1.0, 0.6), false)
                }
                "hit" => (Color::WHITE, f.text.ends_with('!')),
                "hit_fire" => (Color::from_rgb(1.0, 0.55, 0.2), f.text.ends_with('!')),
                "hit_frost" => (Color::from_rgb(0.6, 0.85, 1.0), f.text.ends_with('!')),
                "hit_poison" => (Color::from_rgb(0.6, 1.0, 0.3), f.text.ends_with('!')),
                "hit_shadow" => (Color::from_rgb(0.7, 0.5, 1.0), f.text.ends_with('!')),
                "hit_storm" => (Color::from_rgb(1.0, 0.95, 0.4), f.text.ends_with('!')),
                "hit_holy" => (Color::from_rgb(1.0, 0.95, 0.75), f.text.ends_with('!')),
                "hit_arcane" => (Color::from_rgb(0.75, 0.6, 1.0), f.text.ends_with('!')),
                "hurt" => (Color::from_rgb(1.0, 0.35, 0.3), false),
                "heal" => (Color::from_rgb(0.4, 1.0, 0.4), false),
                "xp" => (Color::from_rgb(0.5, 0.75, 1.0), false),
                "gold" => (Color::from_rgb(1.0, 0.85, 0.2), false),
                "status" => (Color::from_rgb(1.0, 0.75, 1.0), false),
                "level" => (Color::from_rgb(1.0, 0.9, 0.3), true),
                _ => (Color::from_rgb(0.95, 0.95, 0.95), false),
            };
            if f.kind == "level" {
                self.effects
                    .nova(pos, 1.5, Color::from_rgba(1.0, 0.9, 0.3, 0.9));
            }
            if !f.text.is_empty() {
                self.effects.text(pos, &f.text, color, big);
            }
        }
    }

    pub fn update(&mut self, delta: f32, session: &Session) {
        self.terrain.update(delta);
        self.resources.update(delta);
        self.entities.update(delta);
        for (pos, yaw, vertical, delay, color) in self.entities.slashes.drain(..) {
            self.effects.slash(pos, yaw, vertical, delay, color);
        }
        self.effects.update(delta);
        if let Some(p) = self.entities.player_position(session.my_id) {
            let underground = p.y < -5.0;
            let (fog, ambient, density) = match session
                .zone_at(p.x, p.z)
                .and_then(|z| session.catalog.zone(z))
            {
                Some(z) => {
                    let d = match z.biome.as_str() {
                        "swamp" => 0.02,
                        "ash" => 0.016,
                        "snow" => 0.011,
                        "forest" => 0.01,
                        _ => 0.006,
                    };
                    (parse_color(&z.fog), parse_color(&z.ambient), d)
                }
                None => (
                    Color::from_rgb(0.72, 0.75, 0.8),
                    Color::from_rgb(0.55, 0.6, 0.7),
                    0.006,
                ),
            };
            self.lighting
                .update(delta, session.you.tod, underground, fog, ambient, density);
            self.town.update(session.you.tod, self.lighting.night);
            self.grade.update(self.lighting.night);
            let (size, codes) = (session.size, &session.terrain);
            let blocked = |x: f32, z: f32| -> bool {
                if x < 0.0 || z < 0.0 || x >= size as f32 || z >= size as f32 {
                    return false;
                }
                matches!(codes[z as usize * size + x as usize], 6 | 14 | 17)
            };
            self.camera.update(delta, p, &blocked);
        }
    }

    pub fn destroy(mut self) {
        self.entities.clear();
        self.root.queue_free();
    }
}
