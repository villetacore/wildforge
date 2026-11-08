//! The 3D world view: everything drawn in the scene for one game session.
//! It owns Godot nodes under a single root and is rebuilt on every login.

pub mod camera;
pub mod effects;
pub mod entities;
pub mod environment;
pub mod models;
pub mod resources;
pub mod terrain;

use godot::prelude::*;

use crate::content::ContentLibrary;
use crate::net::protocol::Fx;
use crate::net::Catalog;
use crate::session::Session;
use camera::CameraRig;
use effects::Effects;
use entities::{to_world, Entities};
use resources::ResourceField;
use terrain::Terrain;

pub struct WorldView {
    root: Gd<Node3D>,
    terrain: Terrain,
    resources: ResourceField,
    pub entities: Entities,
    effects: Effects,
    pub camera: CameraRig,
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
        environment::build(&mut root);
        let terrain = Terrain::build(&mut root, session.size, &session.terrain, lib);
        let resources = ResourceField::build(
            &mut root,
            session.size,
            &session.nodes,
            depleted,
            &session.catalog,
            lib,
        );
        let entities = Entities::new(&mut root);
        let effects = Effects::new(&mut root);
        let camera = CameraRig::new(&mut root);
        WorldView {
            root,
            terrain,
            resources,
            entities,
            effects,
            camera,
        }
    }

    pub fn set_node_alive(&mut self, tile: usize, alive: bool) {
        self.resources.set_alive(tile, alive);
    }

    pub fn play_fx(&mut self, fx: &[Fx], _catalog: &Catalog) {
        for f in fx {
            let pos = to_world(f.x, f.y);
            let (color, big) = match f.kind.as_str() {
                "swing" => {
                    self.entities.swing_at(f.x, f.y);
                    self.effects.ring(pos, Color::from_rgba(1.0, 1.0, 1.0, 0.7));
                    continue;
                }
                "gather" => {
                    self.resources.shake_at(f.x, f.y);
                    (Color::from_rgb(0.75, 1.0, 0.6), false)
                }
                "hit" => (Color::WHITE, f.text.ends_with('!')),
                "hurt" => (Color::from_rgb(1.0, 0.35, 0.3), false),
                "heal" => (Color::from_rgb(0.4, 1.0, 0.4), false),
                "xp" => (Color::from_rgb(0.5, 0.75, 1.0), false),
                "gold" => (Color::from_rgb(1.0, 0.85, 0.2), false),
                "level" => (Color::from_rgb(1.0, 0.9, 0.3), true),
                _ => (Color::from_rgb(0.95, 0.95, 0.95), false),
            };
            if f.kind == "level" {
                self.effects.ring(pos, Color::from_rgba(1.0, 0.9, 0.3, 0.9));
            }
            self.effects.text(pos, &f.text, color, big);
        }
    }

    pub fn update(&mut self, delta: f32, my_id: i64) {
        self.terrain.update(delta);
        self.resources.update(delta);
        self.entities.update(delta);
        self.effects.update(delta);
        if let Some(p) = self.entities.player_position(my_id) {
            self.camera.update(delta, p);
        }
    }

    pub fn destroy(mut self) {
        self.entities.clear();
        self.root.queue_free();
    }
}
