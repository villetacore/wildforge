//! Terrain: textured ground tiles in 32x32 chunks (one mesh per terrain type
//! per chunk), coastline walls, and an animated ocean plane underneath.

use godot::classes::base_material_3d::CullMode;
use godot::classes::mesh::PrimitiveType;
use godot::classes::{Material, MeshInstance3D, Node3D, StandardMaterial3D, SurfaceTool};
use godot::prelude::*;

use crate::content::ContentLibrary;

pub const WATER: u8 = 1;
const CHUNK: usize = 32;
const COAST_DEPTH: f32 = 0.35;

/// Texture used for each terrain code sent by the server.
pub const TERRAIN_TEXTURES: [&str; 10] = [
    "grass", "water", "sand", "forest", "road", "rocky", "stone", "road", "road", "road",
];

/// Minimap colors per terrain code.
pub fn terrain_color(code: u8) -> Color {
    match code {
        0 => Color::from_rgb(0.36, 0.62, 0.30),
        1 => Color::from_rgb(0.18, 0.38, 0.72),
        2 => Color::from_rgb(0.86, 0.80, 0.55),
        3 => Color::from_rgb(0.22, 0.42, 0.20),
        4 => Color::from_rgb(0.62, 0.52, 0.36),
        5 => Color::from_rgb(0.48, 0.46, 0.42),
        6 => Color::from_rgb(0.12, 0.12, 0.14),
        7 => Color::from_rgb(0.35, 0.32, 0.29),
        8 => Color::from_rgb(0.3, 0.8, 0.9),
        9 => Color::from_rgb(0.7, 0.5, 0.2),
        _ => Color::MAGENTA,
    }
}

fn shade(x: usize, y: usize) -> f32 {
    let h = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
    let h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    0.88 + ((h >> 16) & 0xFF) as f32 / 255.0 * 0.2
}

pub struct Terrain {
    pub water: Gd<StandardMaterial3D>,
    t: f32,
}

impl Terrain {
    pub fn build(
        parent: &mut Gd<Node3D>,
        size: usize,
        codes: &[u8],
        lib: &ContentLibrary,
    ) -> Terrain {
        let mut root = Node3D::new_alloc();
        root.set_name("Terrain");
        let at = |x: i64, y: i64| -> u8 {
            if x < 0 || y < 0 || x >= size as i64 || y >= size as i64 {
                WATER
            } else {
                codes[y as usize * size + x as usize]
            }
        };

        let mats: Vec<Gd<StandardMaterial3D>> = TERRAIN_TEXTURES
            .iter()
            .map(|t| {
                let mut m = lib
                    .material(t, Color::WHITE, 1.0, true)
                    .duplicate_resource();
                m.set_cull_mode(CullMode::DISABLED);
                m
            })
            .collect();

        for cy in (0..size).step_by(CHUNK) {
            for cx in (0..size).step_by(CHUNK) {
                for (code, mat) in mats.iter().enumerate() {
                    let code = code as u8;
                    if code == WATER || code == 6 {
                        continue;
                    }
                    let mut st = SurfaceTool::new_gd();
                    st.begin(PrimitiveType::TRIANGLES);
                    let mut quads = 0;
                    for y in cy..(cy + CHUNK).min(size) {
                        for x in cx..(cx + CHUNK).min(size) {
                            if codes[y * size + x] != code {
                                continue;
                            }
                            quads += 1;
                            let (fx, fy) = (x as f32, y as f32);
                            let v = |st: &mut Gd<SurfaceTool>, vx: usize, vy: usize| {
                                let s = shade(vx, vy);
                                st.set_color(Color::from_rgb(s, s, s));
                                st.set_normal(Vector3::UP);
                                st.set_uv(Vector2::new(vx as f32 * 0.5, vy as f32 * 0.5));
                                st.add_vertex(Vector3::new(
                                    vx as f32,
                                    height(vx as f32, vy as f32),
                                    vy as f32,
                                ));
                            };
                            for (vx, vy) in [
                                (x, y),
                                (x + 1, y),
                                (x + 1, y + 1),
                                (x, y),
                                (x + 1, y + 1),
                                (x, y + 1),
                            ] {
                                v(&mut st, vx, vy);
                            }
                            // Coast walls where land meets water.
                            let (ix, iy) = (x as i64, y as i64);
                            let walls = [
                                (
                                    at(ix, iy - 1),
                                    Vector3::new(fx, 0.0, fy),
                                    Vector3::new(fx + 1.0, 0.0, fy),
                                    Vector3::BACK * -1.0,
                                ),
                                (
                                    at(ix, iy + 1),
                                    Vector3::new(fx, 0.0, fy + 1.0),
                                    Vector3::new(fx + 1.0, 0.0, fy + 1.0),
                                    Vector3::BACK,
                                ),
                                (
                                    at(ix - 1, iy),
                                    Vector3::new(fx, 0.0, fy),
                                    Vector3::new(fx, 0.0, fy + 1.0),
                                    Vector3::LEFT,
                                ),
                                (
                                    at(ix + 1, iy),
                                    Vector3::new(fx + 1.0, 0.0, fy),
                                    Vector3::new(fx + 1.0, 0.0, fy + 1.0),
                                    Vector3::RIGHT,
                                ),
                            ];
                            for (n, a, b, normal) in walls {
                                if n != WATER {
                                    continue;
                                }
                                let down = Vector3::new(0.0, -COAST_DEPTH, 0.0);
                                let len = 1.0;
                                for (p, uv) in [
                                    (a, Vector2::new(0.0, 0.0)),
                                    (b, Vector2::new(len * 0.5, 0.0)),
                                    (b + down, Vector2::new(len * 0.5, COAST_DEPTH * 0.5)),
                                    (a, Vector2::new(0.0, 0.0)),
                                    (b + down, Vector2::new(len * 0.5, COAST_DEPTH * 0.5)),
                                    (a + down, Vector2::new(0.0, COAST_DEPTH * 0.5)),
                                ] {
                                    st.set_color(Color::from_rgb(0.7, 0.66, 0.6));
                                    st.set_normal(normal);
                                    st.set_uv(uv);
                                    st.add_vertex(p);
                                }
                            }
                        }
                    }
                    if quads == 0 {
                        continue;
                    }
                    st.generate_normals();
                    if let Some(mesh) = st.commit() {
                        let mut mi = MeshInstance3D::new_alloc();
                        mi.set_mesh(&mesh);
                        mi.set_material_override(&mat.clone().upcast::<Material>());
                        root.add_child(&mi);
                    }
                }
            }
        }

        let mut water = lib
            .material("water", Color::from_rgb(0.50, 0.58, 0.62), 1.0, false)
            .duplicate_resource();
        water.set_roughness(0.65);
        let mut ocean_mesh = SurfaceTool::new_gd();
        ocean_mesh.begin(PrimitiveType::TRIANGLES);
        for y in 0..size {
            for x in 0..size {
                if codes[y * size + x] != WATER {
                    continue;
                }
                for (dx, dy) in [(0, 0), (1, 1), (1, 0), (0, 0), (0, 1), (1, 1)] {
                    ocean_mesh.set_normal(Vector3::UP);
                    ocean_mesh.set_uv(Vector2::new((x + dx) as f32 / 3.0, (y + dy) as f32 / 3.0));
                    ocean_mesh.add_vertex(Vector3::new((x + dx) as f32, -0.28, (y + dy) as f32));
                }
            }
        }
        if let Some(mesh) = ocean_mesh.commit() {
            let mut ocean = MeshInstance3D::new_alloc();
            ocean.set_mesh(&mesh);
            ocean.set_material_override(&water);
            root.add_child(&ocean);
        }
        super::structures::build(&mut root, size, codes, lib);

        parent.add_child(&root);
        Terrain { water, t: 0.0 }
    }

    /// Scrolls the water texture.
    pub fn update(&mut self, delta: f32) {
        self.t += delta;
        let o = self.t * 0.02;
        self.water.set_uv1_offset(Vector3::new(o, o * 0.6, 0.0));
    }
}

// Height data belongs to the currently displayed world on Godot's main thread.
// Shared sampling keeps entities, resources, effects and mesh vertices aligned.
thread_local! {
    static HEIGHTS: std::cell::RefCell<(usize, Vec<f32>)> = const { std::cell::RefCell::new((0, Vec::new())) };
}

pub fn set_heights(size: usize, values: &[f32]) {
    HEIGHTS.with(|h| *h.borrow_mut() = (size, values.to_vec()));
}

pub fn height(x: f32, y: f32) -> f32 {
    HEIGHTS.with(|h| {
        let h = h.borrow();
        let n = h.0;
        if n == 0 || h.1.len() != (n + 1) * (n + 1) {
            return 0.0;
        }
        let x = x.clamp(0.0, n as f32 - 0.001);
        let y = y.clamp(0.0, n as f32 - 0.001);
        let ix = x.floor() as usize;
        let iy = y.floor() as usize;
        let u = x - ix as f32;
        let v = y - iy as f32;
        let a = h.1[iy * (n + 1) + ix];
        let b = h.1[iy * (n + 1) + ix + 1];
        let c = h.1[(iy + 1) * (n + 1) + ix + 1];
        let d = h.1[(iy + 1) * (n + 1) + ix];
        if u >= v {
            a + u * (b - a) + v * (c - b)
        } else {
            a + u * (c - d) + v * (d - a)
        }
    })
}
