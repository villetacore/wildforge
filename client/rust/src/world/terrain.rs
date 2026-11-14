//! Terrain: textured ground tiles in 32x32 chunks (one mesh per terrain type
//! per chunk), coastline walls, an animated water surface, glowing lava,
//! instanced grass and bridge parapets. Dungeon squares are skipped here;
//! `structures::DungeonView` draws (and redraws) them.

use godot::classes::base_material_3d::{CullMode, Feature};
use godot::classes::geometry_instance_3d::{ShadowCastingSetting, VisibilityRangeFadeMode};
use godot::classes::mesh::PrimitiveType;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    Material, Mesh, MeshInstance3D, MultiMesh, MultiMeshInstance3D, Node3D, Shader, ShaderMaterial,
    StandardMaterial3D, SurfaceTool,
};
use godot::prelude::*;

use super::models;
use crate::content::ContentLibrary;
use crate::net::protocol::DungeonInfo;

pub const WATER: u8 = 1;
pub const LAVA: u8 = 15;
const CHUNK: usize = 32;
const COAST_DEPTH: f32 = 0.35;

/// Texture used for each terrain code sent by the server.
pub const TERRAIN_TEXTURES: [&str; 18] = [
    "grass",
    "water",
    "sand",
    "forest",
    "road",
    "rocky",
    "stone",
    "road",
    "road",
    "road",
    "snow",
    "swamp",
    "ash",
    "cobble",
    "cobble",
    "lava",
    "stone_blocks",
    "cobble",
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
        10 => Color::from_rgb(0.92, 0.94, 0.98),
        11 => Color::from_rgb(0.3, 0.38, 0.22),
        12 => Color::from_rgb(0.25, 0.22, 0.21),
        13 => Color::from_rgb(0.62, 0.6, 0.56),
        14 => Color::from_rgb(0.72, 0.36, 0.26),
        15 => Color::from_rgb(1.0, 0.4, 0.05),
        16 => Color::from_rgb(0.7, 0.66, 0.6),
        17 => Color::from_rgb(0.3, 0.28, 0.26),
        _ => Color::MAGENTA,
    }
}

const WATER_SHADER: &str = r#"
shader_type spatial;
render_mode blend_mix, cull_back, specular_schlick_ggx;
uniform vec3 deep : source_color = vec3(0.04, 0.16, 0.26);
uniform vec3 shallow : source_color = vec3(0.12, 0.42, 0.5);
uniform vec3 foam : source_color = vec3(0.85, 0.92, 0.95);
uniform sampler2D ripples : hint_normal, filter_linear_mipmap, repeat_enable;
varying float wave;
void vertex() {
    float t = TIME;
    vec3 w = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
    wave = sin(w.x * 0.7 + t * 1.2) * 0.5 + cos(w.z * 0.55 + t * 0.9) * 0.5;
    VERTEX.y += wave * 0.05;
}
void fragment() {
    vec2 uv = UV;
    vec3 n1 = texture(ripples, uv + vec2(TIME * 0.012, TIME * 0.008)).rgb;
    vec3 n2 = texture(ripples, uv * 1.7 - vec2(TIME * 0.01, TIME * 0.016)).rgb;
    NORMAL_MAP = normalize(mix(n1, n2, 0.5));
    NORMAL_MAP_DEPTH = 0.6;
    float fres = pow(1.0 - clamp(dot(NORMAL, VIEW), 0.0, 1.0), 5.0);
    vec3 col = mix(deep, shallow, 0.35 + 0.3 * wave);
    col = mix(col, foam, smoothstep(0.85, 1.0, wave) * 0.15);
    ALBEDO = col + fres * 0.25;
    ROUGHNESS = 0.06;
    METALLIC = 0.0;
    SPECULAR = 0.8;
}
"#;

const LAVA_SHADER: &str = r#"
shader_type spatial;
render_mode cull_back;
uniform sampler2D lava : source_color, filter_linear_mipmap, repeat_enable;
void fragment() {
    vec2 uv = UV * 0.5 + vec2(TIME * 0.01, sin(TIME * 0.2) * 0.02);
    vec3 c = texture(lava, uv).rgb;
    float pulse = 0.8 + 0.2 * sin(TIME * 1.5 + UV.x * 3.0);
    ALBEDO = c * 0.3;
    EMISSION = c * c * 3.0 * pulse;
    ROUGHNESS = 0.9;
}
"#;

fn shade(x: usize, y: usize) -> f32 {
    let h = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
    let h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    0.9 + ((h >> 16) & 0xFF) as f32 / 255.0 * 0.16
}

fn rand01(i: usize, salt: u32) -> f32 {
    let h = (i as u32).wrapping_mul(2_654_435_761) ^ salt.wrapping_mul(40_503);
    let h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
    ((h ^ (h >> 13)) & 0xFFFF) as f32 / 65535.0
}

pub fn in_dungeon(dungeons: &[DungeonInfo], x: usize, y: usize) -> bool {
    dungeons
        .iter()
        .any(|d| x >= d.x && y >= d.y && x < d.x + d.size && y < d.y + d.size)
}

/// A small tuft of grass blades for instancing.
fn tuft() -> Gd<Mesh> {
    let mut st = SurfaceTool::new_gd();
    st.begin(PrimitiveType::TRIANGLES);
    for k in 0..5 {
        let a = k as f32 / 5.0 * std::f32::consts::TAU + 0.3;
        let (dx, dz) = (a.cos(), a.sin());
        let lean = Vector3::new(dx * 0.12, 0.0, dz * 0.12);
        let side = Vector3::new(-dz, 0.0, dx) * 0.035;
        let h = 0.28 + (k as f32 * 0.37).sin().abs() * 0.18;
        let base = Vector3::new(dx * 0.05, 0.0, dz * 0.05);
        for (p, c) in [
            (base - side, 0.55),
            (base + side, 0.55),
            (base + lean + Vector3::new(0.0, h, 0.0), 1.0),
        ] {
            st.set_color(Color::from_rgb(c, c, c));
            st.set_normal(Vector3::UP);
            st.set_uv(Vector2::new(0.5, 1.0 - c));
            st.add_vertex(p);
        }
    }
    st.commit().unwrap().upcast()
}

pub struct Terrain {
    t: f32,
}

impl Terrain {
    pub fn build(
        parent: &mut Gd<Node3D>,
        size: usize,
        codes: &[u8],
        dungeons: &[DungeonInfo],
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
        let mut lava_shader = Shader::new_gd();
        lava_shader.set_code(LAVA_SHADER);
        let mut lava_mat = ShaderMaterial::new_gd();
        lava_mat.set_shader(&lava_shader);
        if let Some(t) = lib.texture("lava") {
            lava_mat.set_shader_parameter("lava", &t.to_variant());
        }

        let grass_mesh = tuft();
        let mut grass_mat = StandardMaterial3D::new_gd();
        grass_mat.set_flag(
            godot::classes::base_material_3d::Flags::ALBEDO_FROM_VERTEX_COLOR,
            true,
        );
        grass_mat.set_cull_mode(CullMode::DISABLED);
        grass_mat.set_roughness(0.9);
        let mut parapets: Vec<Transform3D> = Vec::new();

        for cy in (0..size).step_by(CHUNK) {
            for cx in (0..size).step_by(CHUNK) {
                let mut grass: Vec<(Transform3D, Color)> = Vec::new();
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
                            let c = codes[y * size + x];
                            if c != code || in_dungeon(dungeons, x, y) {
                                continue;
                            }
                            quads += 1;
                            let (fx, fy) = (x as f32, y as f32);
                            let v = |st: &mut Gd<SurfaceTool>, vx: usize, vy: usize| {
                                let s = shade(vx, vy);
                                st.set_color(Color::from_rgb(s, s, s));
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
                            // Grass and flowers on green ground.
                            let i = y * size + x;
                            if matches!(c, 0 | 3 | 11) {
                                let n = if c == 3 { 2 } else { 3 };
                                for k in 0..n {
                                    let gx = fx + rand01(i, 10 + k) * 0.95;
                                    let gz = fy + rand01(i, 20 + k) * 0.95;
                                    let s = 0.4 + rand01(i, 30 + k) * 0.4;
                                    let tint = match c {
                                        11 => Color::from_rgb(0.35, 0.45, 0.2),
                                        3 => Color::from_rgb(0.25, 0.45, 0.18),
                                        _ => Color::from_rgb(
                                            0.42 + rand01(i, 40 + k) * 0.15,
                                            0.68,
                                            0.28,
                                        ),
                                    };
                                    grass.push((
                                        models::trs(
                                            Vector3::new(gx, height(gx, gz) - 0.02, gz),
                                            rand01(i, 50 + k) * std::f32::consts::TAU,
                                            Vector3::new(s, s * (0.8 + rand01(i, 60 + k) * 0.6), s),
                                        ),
                                        tint,
                                    ));
                                }
                                if c == 0 && rand01(i, 70) > 0.93 {
                                    let gx = fx + rand01(i, 71);
                                    let gz = fy + rand01(i, 72);
                                    let flower = [
                                        Color::from_rgb(1.0, 0.9, 0.3),
                                        Color::from_rgb(0.9, 0.4, 0.6),
                                        Color::from_rgb(0.6, 0.6, 1.0),
                                        Color::from_rgb(1.0, 1.0, 1.0),
                                    ][(rand01(i, 73) * 3.99) as usize];
                                    grass.push((
                                        models::trs(
                                            Vector3::new(gx, height(gx, gz), gz),
                                            0.0,
                                            Vector3::new(0.5, 1.4, 0.5),
                                        ),
                                        flower * 2.0,
                                    ));
                                }
                            }
                            // Coast walls where land meets water.
                            let (ix, iy) = (x as i64, y as i64);
                            let walls = [
                                (
                                    at(ix, iy - 1),
                                    Vector3::new(fx, 0.0, fy),
                                    Vector3::new(fx + 1.0, 0.0, fy),
                                    Vector3::FORWARD,
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
                                if c == 16 {
                                    // Bridge parapet along the water side.
                                    let mid = (a + b) * 0.5 - normal * 0.12;
                                    let along = b - a;
                                    parapets.push(Transform3D::new(
                                        Basis::from_cols(
                                            along,
                                            Vector3::UP,
                                            along.cross(Vector3::UP),
                                        ),
                                        Vector3::new(mid.x, height(mid.x, mid.z) + 0.3, mid.z),
                                    ));
                                }
                                let down = Vector3::new(0.0, -COAST_DEPTH, 0.0);
                                for (p, uv) in [
                                    (a, Vector2::new(0.0, 0.0)),
                                    (b, Vector2::new(0.5, 0.0)),
                                    (b + down, Vector2::new(0.5, COAST_DEPTH * 0.5)),
                                    (a, Vector2::new(0.0, 0.0)),
                                    (b + down, Vector2::new(0.5, COAST_DEPTH * 0.5)),
                                    (a + down, Vector2::new(0.0, COAST_DEPTH * 0.5)),
                                ] {
                                    st.set_color(Color::from_rgb(0.7, 0.66, 0.6));
                                    st.set_uv(uv);
                                    st.add_vertex(p);
                                }
                            }
                        }
                    }
                    if quads == 0 {
                        continue;
                    }
                    st.index();
                    st.generate_normals();
                    st.generate_tangents();
                    if let Some(mesh) = st.commit() {
                        let mut mi = MeshInstance3D::new_alloc();
                        mi.set_mesh(&mesh);
                        if code == LAVA {
                            mi.set_material_override(&lava_mat.clone().upcast::<Material>());
                        } else {
                            mi.set_material_override(&mat.clone().upcast::<Material>());
                        }
                        root.add_child(&mi);
                    }
                }
                if !grass.is_empty() {
                    let mut mm = MultiMesh::new_gd();
                    mm.set_transform_format(TransformFormat::TRANSFORM_3D);
                    mm.set_use_colors(true);
                    mm.set_mesh(&grass_mesh);
                    mm.set_instance_count(grass.len() as i32);
                    for (k, (tf, c)) in grass.iter().enumerate() {
                        mm.set_instance_transform(k as i32, *tf);
                        mm.set_instance_color(k as i32, *c);
                    }
                    let mut mmi = MultiMeshInstance3D::new_alloc();
                    mmi.set_multimesh(&mm);
                    mmi.set_material_override(&grass_mat.clone().upcast::<Material>());
                    mmi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
                    mmi.set_visibility_range_end(55.0);
                    mmi.set_visibility_range_end_margin(8.0);
                    mmi.set_visibility_range_fade_mode(VisibilityRangeFadeMode::SELF);
                    root.add_child(&mmi);
                }
            }
        }

        // Water: one surface with a ripple shader, slightly below the land.
        let mut shader = Shader::new_gd();
        shader.set_code(WATER_SHADER);
        let mut water = ShaderMaterial::new_gd();
        water.set_shader(&shader);
        if let Some(n) = lib.normal_map("water") {
            water.set_shader_parameter("ripples", &n.to_variant());
        }
        let mut ocean_mesh = SurfaceTool::new_gd();
        ocean_mesh.begin(PrimitiveType::TRIANGLES);
        let mut any = false;
        for y in 0..size {
            for x in 0..size {
                if codes[y * size + x] != WATER && codes[y * size + x] != 16 {
                    continue;
                }
                any = true;
                for (dx, dy) in [(0, 0), (1, 1), (1, 0), (0, 0), (0, 1), (1, 1)] {
                    ocean_mesh.set_normal(Vector3::UP);
                    ocean_mesh.set_uv(Vector2::new((x + dx) as f32 / 6.0, (y + dy) as f32 / 6.0));
                    ocean_mesh.add_vertex(Vector3::new((x + dx) as f32, -0.28, (y + dy) as f32));
                }
            }
        }
        // A wide sea beyond the map edge so the island floats in an ocean.
        let n = size as f32;
        for (x0, z0, x1, z1) in [
            (-200.0, -200.0, n + 200.0, 0.0),
            (-200.0, n, n + 200.0, n + 200.0),
            (-200.0, 0.0, 0.0, n),
            (n, 0.0, n + 200.0, n),
        ] {
            for (vx, vz) in [(x0, z0), (x1, z1), (x1, z0), (x0, z0), (x0, z1), (x1, z1)] {
                ocean_mesh.set_normal(Vector3::UP);
                ocean_mesh.set_uv(Vector2::new(vx / 6.0, vz / 6.0));
                ocean_mesh.add_vertex(Vector3::new(vx, -0.28, vz));
            }
        }
        if any || size > 0 {
            ocean_mesh.generate_tangents();
            if let Some(mesh) = ocean_mesh.commit() {
                let mut ocean = MeshInstance3D::new_alloc();
                ocean.set_mesh(&mesh);
                ocean.set_material_override(&water.upcast::<Material>());
                ocean.set_cast_shadows_setting(ShadowCastingSetting::OFF);
                root.add_child(&ocean);
            }
        }
        if !parapets.is_empty() {
            let mut mm = MultiMesh::new_gd();
            mm.set_transform_format(TransformFormat::TRANSFORM_3D);
            mm.set_mesh(&models::cube(Vector3::new(1.0, 0.6, 0.25)));
            mm.set_instance_count(parapets.len() as i32);
            for (k, tf) in parapets.iter().enumerate() {
                mm.set_instance_transform(k as i32, *tf);
            }
            let mut mmi = MultiMeshInstance3D::new_alloc();
            mmi.set_multimesh(&mm);
            mmi.set_material_override(
                &lib.material_world("stone_blocks", Color::from_rgb(0.75, 0.7, 0.62), 0.45)
                    .upcast::<Material>(),
            );
            root.add_child(&mmi);
        }
        let _ = Feature::EMISSION;
        parent.add_child(&root);
        Terrain { t: 0.0 }
    }

    pub fn update(&mut self, delta: f32) {
        self.t += delta;
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
