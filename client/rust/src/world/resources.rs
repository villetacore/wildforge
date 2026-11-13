//! Gatherable resource nodes (trees, rocks, ores, crystals) drawn with one
//! MultiMesh per model part. Depleted nodes are hidden by collapsing their
//! instance transforms; gathering makes a node shake briefly.

use std::collections::{HashMap, HashSet};

use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{Material, MultiMesh, MultiMeshInstance3D, Node3D};
use godot::prelude::*;

use super::models::{self, Part};
use crate::content::{parse_color, ContentLibrary};
use crate::net::Catalog;

const SHAKE_TIME: f32 = 0.35;

struct Kind {
    parts: Vec<Part>,
    multimeshes: Vec<Gd<MultiMesh>>,
    node_tf: Vec<Transform3D>,
}

pub struct ResourceField {
    kinds: Vec<Kind>,
    /// tile index -> (kind, instance)
    slots: HashMap<usize, (usize, usize)>,
    shakes: Vec<(usize, f32)>,
    dead: HashSet<usize>,
    size: usize,
}

fn rand01(i: usize, salt: u32) -> f32 {
    let h = (i as u32).wrapping_mul(2_654_435_761) ^ salt.wrapping_mul(40_503);
    let h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
    ((h ^ (h >> 13)) & 0xFFFF) as f32 / 65535.0
}

fn hidden(tf: Transform3D) -> Transform3D {
    Transform3D::new(Basis::from_scale(Vector3::splat(0.0001)), tf.origin)
}

impl ResourceField {
    pub fn build(
        parent: &mut Gd<Node3D>,
        size: usize,
        nodes: &[u8],
        depleted: &[usize],
        catalog: &Catalog,
        lib: &ContentLibrary,
    ) -> ResourceField {
        let mut root = Node3D::new_alloc();
        root.set_name("Resources");

        let mut per_kind: Vec<Vec<usize>> = vec![Vec::new(); catalog.defs.resources.len()];
        for (i, code) in nodes.iter().enumerate() {
            if let Some(list) = (*code as usize)
                .checked_sub(1)
                .and_then(|k| per_kind.get_mut(k))
            {
                list.push(i);
            }
        }

        let mut kinds = Vec::new();
        let mut slots = HashMap::new();
        for (k, def) in catalog.defs.resources.iter().enumerate() {
            let tiles = &per_kind[k];
            let parts =
                models::resource_parts(&def.model, &def.texture, parse_color(&def.color), lib);
            let node_tf: Vec<Transform3D> = tiles
                .iter()
                .map(|&i| {
                    let (x, y) = ((i % size) as f32 + 0.5, (i / size) as f32 + 0.5);
                    let jitter = Vector3::new(rand01(i, 1) - 0.5, 0.0, rand01(i, 2) - 0.5) * 0.3;
                    let s = 0.8 + rand01(i, 3) * 0.5;
                    models::trs(
                        Vector3::new(x, super::terrain::height(x + jitter.x, y + jitter.z), y)
                            + jitter,
                        rand01(i, 4) * std::f32::consts::TAU,
                        Vector3::splat(s),
                    )
                })
                .collect();
            let mut multimeshes = Vec::new();
            for part in &parts {
                let mut mm = MultiMesh::new_gd();
                mm.set_transform_format(TransformFormat::TRANSFORM_3D);
                mm.set_mesh(&part.mesh);
                mm.set_instance_count(tiles.len() as i32);
                for (n, tf) in node_tf.iter().enumerate() {
                    mm.set_instance_transform(n as i32, *tf * part.local);
                }
                let mut mmi = MultiMeshInstance3D::new_alloc();
                mmi.set_multimesh(&mm);
                mmi.set_material_override(&part.material.clone().upcast::<Material>());
                root.add_child(&mmi);
                multimeshes.push(mm);
            }
            for (n, &i) in tiles.iter().enumerate() {
                slots.insert(i, (k, n));
            }
            kinds.push(Kind {
                parts,
                multimeshes,
                node_tf,
            });
        }
        parent.add_child(&root);

        let mut field = ResourceField {
            kinds,
            slots,
            shakes: Vec::new(),
            dead: HashSet::new(),
            size,
        };
        for &i in depleted {
            field.set_alive(i, false);
        }
        field
    }

    fn write(&mut self, tile: usize, tf: Option<Transform3D>) {
        let Some(&(k, n)) = self.slots.get(&tile) else {
            return;
        };
        if self.dead.contains(&tile) {
            return;
        }
        let kind = &mut self.kinds[k];
        let base = tf.unwrap_or(kind.node_tf[n]);
        for (part, mm) in kind.parts.iter().zip(kind.multimeshes.iter_mut()) {
            mm.set_instance_transform(n as i32, base * part.local);
        }
    }

    pub fn set_alive(&mut self, tile: usize, alive: bool) {
        let Some(&(k, n)) = self.slots.get(&tile) else {
            return;
        };
        self.shakes.retain(|s| s.0 != tile);
        if alive {
            self.dead.remove(&tile);
            self.write(tile, None);
        } else {
            self.dead.insert(tile);
            let tf = self.kinds[k].node_tf[n];
            let kind = &mut self.kinds[k];
            for (part, mm) in kind.parts.iter().zip(kind.multimeshes.iter_mut()) {
                mm.set_instance_transform(n as i32, hidden(tf * part.local));
            }
        }
    }

    /// Starts a shake on the node nearest to world position (x, y).
    pub fn shake_at(&mut self, x: f32, y: f32) {
        let tile = (y.floor().max(0.0) as usize) * self.size + x.floor().max(0.0) as usize;
        if self.slots.contains_key(&tile)
            && !self.dead.contains(&tile)
            && !self.shakes.iter().any(|s| s.0 == tile)
        {
            self.shakes.push((tile, 0.0));
        }
    }

    pub fn update(&mut self, delta: f32) {
        let mut shakes = std::mem::take(&mut self.shakes);
        for (tile, t) in shakes.iter_mut() {
            *t += delta;
            let Some(&(k, n)) = self.slots.get(tile) else {
                continue;
            };
            let base = self.kinds[k].node_tf[n];
            let k01 = (1.0 - *t / SHAKE_TIME).max(0.0);
            let angle = (*t * 45.0).sin() * 0.12 * k01;
            let tilt = Basis::from_axis_angle(Vector3::FORWARD, angle);
            self.write(
                *tile,
                Some(Transform3D::new(tilt * base.basis, base.origin)),
            );
        }
        shakes.retain(|(tile, t)| {
            if *t >= SHAKE_TIME {
                self.write(*tile, None);
                false
            } else {
                true
            }
        });
        self.shakes = shakes;
    }
}
