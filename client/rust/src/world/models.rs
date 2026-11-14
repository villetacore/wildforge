//! Procedural models built from Godot primitive meshes: resource nodes,
//! customisable humanoids, monsters, pets and ground items. Meshes are
//! cached per shape so thousands of actors share geometry.

use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use godot::classes::base_material_3d::{CullMode, Feature, ShadingMode, Transparency};
use godot::classes::light_3d::Param;
use godot::classes::{
    BoxMesh, CapsuleMesh, CylinderMesh, Material, Mesh, MeshInstance3D, Node3D, OmniLight3D,
    PrimitiveMesh, SphereMesh, StandardMaterial3D, SurfaceTool, TorusMesh,
};
use godot::prelude::*;

use crate::content::{parse_color, ContentLibrary};
use crate::net::protocol::Look;

pub fn at(pos: Vector3) -> Transform3D {
    Transform3D::new(Basis::IDENTITY, pos)
}

pub fn trs(pos: Vector3, yaw: f32, scale: Vector3) -> Transform3D {
    Transform3D::new(
        Basis::from_axis_angle(Vector3::UP, yaw) * Basis::from_scale(scale),
        pos,
    )
}

pub fn rot(pos: Vector3, euler: Vector3, scale: Vector3) -> Transform3D {
    Transform3D::new(
        Basis::from_euler(EulerOrder::YXZ, euler) * Basis::from_scale(scale),
        pos,
    )
}

thread_local! {
    static MESHES: RefCell<HashMap<String, Gd<Mesh>>> = RefCell::new(HashMap::new());
}

fn cached(key: String, make: impl FnOnce() -> Gd<Mesh>) -> Gd<Mesh> {
    if let Some(m) = MESHES.with(|c| c.borrow().get(&key).cloned()) {
        return m;
    }
    let m = make();
    MESHES.with(|c| c.borrow_mut().insert(key, m.clone()));
    m
}

/// Clears the mesh cache; called before the engine shuts down.
pub fn clear_cache() {
    MESHES.with(|c| c.borrow_mut().clear());
}

/// Re-indexes a primitive with flat normals (faceted look for rocks).
fn faceted<T: Inherits<PrimitiveMesh> + Inherits<Mesh>>(m: Gd<T>) -> Gd<Mesh> {
    let mesh = m.upcast::<Mesh>();
    let arrays = mesh.surface_get_arrays(0);
    let vertices = arrays.get(0).unwrap().to::<PackedVector3Array>();
    let uvs = arrays
        .get(4)
        .unwrap()
        .try_to::<PackedVector2Array>()
        .unwrap_or_default();
    let indices = arrays
        .get(12)
        .unwrap()
        .try_to::<PackedInt32Array>()
        .unwrap_or_default();
    let order: Vec<usize> = if indices.is_empty() {
        (0..vertices.len()).collect()
    } else {
        indices.as_slice().iter().map(|i| *i as usize).collect()
    };
    let mut surface = SurfaceTool::new_gd();
    surface.begin(godot::classes::mesh::PrimitiveType::TRIANGLES);
    for triangle in order.chunks_exact(3) {
        let a = vertices[triangle[0]];
        let b = vertices[triangle[1]];
        let c = vertices[triangle[2]];
        let cross = (c - a).cross(b - a);
        if cross.length_squared() < 1e-12 {
            continue;
        }
        let normal = cross.normalized();
        for &i in triangle {
            surface.set_normal(normal);
            if i < uvs.len() {
                surface.set_uv(uvs[i]);
            }
            surface.add_vertex(vertices[i]);
        }
    }
    surface.generate_tangents();
    surface.commit().map(|m| m.upcast::<Mesh>()).unwrap_or(mesh)
}

pub fn cylinder(top: f32, bottom: f32, height: f32, segments: i32) -> Gd<Mesh> {
    cached(format!("cyl{top}|{bottom}|{height}|{segments}"), || {
        let mut m = CylinderMesh::new_gd();
        m.set_top_radius(top);
        m.set_bottom_radius(bottom);
        m.set_height(height);
        m.set_radial_segments(segments);
        m.set_rings(1);
        if segments <= 6 {
            faceted(m)
        } else {
            m.upcast()
        }
    })
}

pub fn sphere(radius: f32, height: f32, segments: i32, rings: i32) -> Gd<Mesh> {
    cached(format!("sph{radius}|{height}|{segments}|{rings}"), || {
        let mut m = SphereMesh::new_gd();
        m.set_radius(radius);
        m.set_height(height);
        m.set_radial_segments(segments);
        m.set_rings(rings);
        if segments <= 8 {
            faceted(m)
        } else {
            m.upcast()
        }
    })
}

pub fn cube(size: Vector3) -> Gd<Mesh> {
    cached(format!("box{}|{}|{}", size.x, size.y, size.z), || {
        let mut m = BoxMesh::new_gd();
        m.set_size(size);
        m.upcast()
    })
}

pub fn capsule(radius: f32, height: f32) -> Gd<Mesh> {
    cached(format!("cap{radius}|{height}"), || {
        let mut m = CapsuleMesh::new_gd();
        m.set_radius(radius);
        m.set_height(height.max(radius * 2.0 + 0.001));
        m.set_radial_segments(14);
        m.set_rings(4);
        m.upcast()
    })
}

pub fn torus(inner: f32, outer: f32) -> Gd<Mesh> {
    cached(format!("tor{inner}|{outer}"), || {
        let mut m = TorusMesh::new_gd();
        m.set_inner_radius(inner);
        m.set_outer_radius(outer);
        m.set_rings(24);
        m.set_ring_segments(8);
        m.upcast()
    })
}

/// A flat blade (for propellers, wings and leaves).
pub fn blade(len: f32, width: f32) -> Gd<Mesh> {
    cached(format!("blade{len}|{width}"), || {
        let mut st = SurfaceTool::new_gd();
        st.begin(godot::classes::mesh::PrimitiveType::TRIANGLES);
        let pts = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(len * 0.5, 0.0, width * 0.5),
            Vector3::new(len, 0.0, 0.0),
            Vector3::new(len * 0.5, 0.0, -width * 0.5),
        ];
        for (i, uv) in [
            (0, (0.0, 0.5)),
            (1, (0.5, 1.0)),
            (2, (1.0, 0.5)),
            (0, (0.0, 0.5)),
            (2, (1.0, 0.5)),
            (3, (0.5, 0.0)),
        ] {
            st.set_normal(Vector3::UP);
            st.set_uv(Vector2::new(uv.0, uv.1));
            st.add_vertex(pts[i]);
        }
        st.commit().unwrap().upcast()
    })
}

/// A gable roof prism: ridge along X, eaves at y = 0, ridge at y = h.
pub fn gable_like(w: f32, d: f32, h: f32) -> Gd<Mesh> {
    cached(format!("gable{w}|{d}|{h}"), || gable_mesh(w, d, h))
}

fn gable_mesh(w: f32, d: f32, h: f32) -> Gd<Mesh> {
    let mut st = SurfaceTool::new_gd();
    st.begin(godot::classes::mesh::PrimitiveType::TRIANGLES);
    let (hw, hd) = (w / 2.0, d / 2.0);
    let a = Vector3::new(-hw, 0.0, -hd);
    let b = Vector3::new(hw, 0.0, -hd);
    let c = Vector3::new(hw, 0.0, hd);
    let dd = Vector3::new(-hw, 0.0, hd);
    let r0 = Vector3::new(-hw, h, 0.0);
    let r1 = Vector3::new(hw, h, 0.0);
    let slope = (hd * hd + h * h).sqrt();
    let mut quad = |p: [Vector3; 4], uvw: f32, uvh: f32| {
        let n = (p[1] - p[0]).cross(p[3] - p[0]).normalized();
        let uv = [
            Vector2::new(0.0, 0.0),
            Vector2::new(uvw, 0.0),
            Vector2::new(uvw, uvh),
            Vector2::new(0.0, uvh),
        ];
        for i in [0, 1, 2, 0, 2, 3] {
            st.set_normal(n);
            st.set_uv(uv[i]);
            st.add_vertex(p[i]);
        }
    };
    quad([r0, r1, b, a], w * 0.5, slope * 0.5);
    quad([r1, r0, dd, c], w * 0.5, slope * 0.5);
    let mut tri = |p: [Vector3; 3]| {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalized();
        for (i, q) in p.iter().enumerate() {
            st.set_normal(n);
            st.set_uv(Vector2::new(q.z * 0.5 + i as f32 * 0.0, q.y * 0.5));
            st.add_vertex(*q);
        }
    };
    tri([a, dd, r0]);
    tri([c, b, r1]);
    st.generate_tangents();
    st.commit().unwrap().upcast()
}

pub fn instance(
    mesh: &Gd<Mesh>,
    mat: &Gd<StandardMaterial3D>,
    tf: Transform3D,
) -> Gd<MeshInstance3D> {
    let mut mi = MeshInstance3D::new_alloc();
    mi.set_mesh(mesh);
    mi.set_material_override(&mat.clone().upcast::<Material>());
    mi.set_transform(tf);
    mi
}

pub fn glow_material(color: Color, energy: f32) -> Gd<StandardMaterial3D> {
    let mut m = StandardMaterial3D::new_gd();
    m.set_albedo(color);
    m.set_feature(Feature::EMISSION, true);
    m.set_emission(color);
    m.set_emission_energy_multiplier(energy);
    m
}

pub fn ghost_material(color: Color) -> Gd<StandardMaterial3D> {
    let mut m = glow_material(color, 2.0);
    m.set_transparency(Transparency::ALPHA);
    m.set_shading_mode(ShadingMode::UNSHADED);
    m.set_cull_mode(CullMode::DISABLED);
    m
}

pub fn light(color: Color, energy: f32, range: f32) -> Gd<OmniLight3D> {
    let mut l = OmniLight3D::new_alloc();
    l.set_color(color);
    l.set_param(Param::ENERGY, energy);
    l.set_param(Param::RANGE, range);
    l
}

// ---------------------------------------------------------------- resources

/// One mesh of a multi-part resource model, placed relative to the node.
pub struct Part {
    pub mesh: Gd<Mesh>,
    pub material: Gd<StandardMaterial3D>,
    pub local: Transform3D,
}

/// Parts of a gatherable resource node model.
pub fn resource_parts(model: &str, texture: &str, color: Color, lib: &ContentLibrary) -> Vec<Part> {
    let white = Color::WHITE;
    let part = |mesh: Gd<Mesh>, material: Gd<StandardMaterial3D>, local: Transform3D| Part {
        mesh,
        material,
        local,
    };
    match model {
        "tree" => {
            let leaves = lib.material("leaves", color.lightened(0.35), 1.5, false);
            vec![
                part(
                    cylinder(0.1, 0.19, 1.5, 9),
                    lib.material(texture, white, 1.0, false),
                    at(Vector3::new(0.0, 0.75, 0.0)),
                ),
                part(
                    sphere(0.8, 1.25, 12, 7),
                    leaves.clone(),
                    at(Vector3::new(0.0, 1.7, 0.0)),
                ),
                part(
                    sphere(0.58, 1.0, 10, 6),
                    leaves.clone(),
                    at(Vector3::new(0.25, 2.3, -0.12)),
                ),
                part(
                    sphere(0.5, 0.8, 10, 6),
                    leaves,
                    at(Vector3::new(-0.3, 2.0, 0.2)),
                ),
            ]
        }
        "pine" => {
            let needles = lib.material("leaves", color.lightened(0.25), 1.0, false);
            let snow = lib.material("snow", white, 1.0, false);
            vec![
                part(
                    cylinder(0.08, 0.15, 1.2, 8),
                    lib.material(texture, white, 1.0, false),
                    at(Vector3::new(0.0, 0.6, 0.0)),
                ),
                part(
                    cylinder(0.0, 0.85, 1.3, 9),
                    needles.clone(),
                    at(Vector3::new(0.0, 1.3, 0.0)),
                ),
                part(
                    cylinder(0.0, 0.65, 1.1, 9),
                    needles.clone(),
                    at(Vector3::new(0.0, 1.95, 0.0)),
                ),
                part(
                    cylinder(0.0, 0.42, 0.9, 9),
                    needles,
                    at(Vector3::new(0.0, 2.55, 0.0)),
                ),
                part(
                    cylinder(0.0, 0.3, 0.35, 9),
                    snow,
                    at(Vector3::new(0.0, 2.85, 0.0)),
                ),
            ]
        }
        "palm" => {
            let leaf = lib.material("leaves", color.lightened(0.3), 1.0, false);
            let mut parts = vec![part(
                cylinder(0.09, 0.14, 2.6, 8),
                lib.material(texture, Color::from_rgb(0.9, 0.8, 0.6), 1.0, false),
                rot(
                    Vector3::new(0.15, 1.3, 0.0),
                    Vector3::new(0.0, 0.0, -0.12),
                    Vector3::ONE,
                ),
            )];
            for i in 0..6 {
                let a = i as f32 / 6.0 * TAU;
                parts.push(part(
                    blade(1.3, 0.4),
                    leaf.clone(),
                    rot(
                        Vector3::new(0.3, 2.6, 0.0),
                        Vector3::new(0.0, a, -0.45),
                        Vector3::ONE,
                    ),
                ));
            }
            parts
        }
        "deadtree" => {
            let bark = lib.material(texture, color.lightened(0.2), 1.0, false);
            vec![
                part(
                    cylinder(0.1, 0.2, 1.6, 7),
                    bark.clone(),
                    at(Vector3::new(0.0, 0.8, 0.0)),
                ),
                part(
                    cylinder(0.03, 0.07, 0.9, 6),
                    bark.clone(),
                    rot(
                        Vector3::new(0.25, 1.5, 0.0),
                        Vector3::new(0.0, 0.0, -0.8),
                        Vector3::ONE,
                    ),
                ),
                part(
                    cylinder(0.03, 0.06, 0.8, 6),
                    bark.clone(),
                    rot(
                        Vector3::new(-0.2, 1.3, 0.1),
                        Vector3::new(0.3, 0.0, 0.9),
                        Vector3::ONE,
                    ),
                ),
                part(
                    cylinder(0.02, 0.05, 0.6, 6),
                    bark,
                    rot(
                        Vector3::new(0.0, 1.75, -0.2),
                        Vector3::new(-0.7, 0.0, 0.0),
                        Vector3::ONE,
                    ),
                ),
            ]
        }
        "herb" => {
            let stem = lib.material("leaves", Color::from_rgb(0.5, 0.8, 0.4), 1.0, false);
            let bloom = glow_material(color, 0.4);
            let mut parts = Vec::new();
            for i in 0..5 {
                let a = i as f32 / 5.0 * TAU;
                parts.push(part(
                    blade(0.4, 0.12),
                    stem.clone(),
                    rot(
                        Vector3::new(0.0, 0.05, 0.0),
                        Vector3::new(0.0, a, -0.9),
                        Vector3::ONE,
                    ),
                ));
            }
            parts.push(part(
                sphere(0.07, 0.12, 10, 5),
                bloom.clone(),
                at(Vector3::new(0.05, 0.3, 0.0)),
            ));
            parts.push(part(
                sphere(0.06, 0.1, 10, 5),
                bloom,
                at(Vector3::new(-0.08, 0.25, 0.06)),
            ));
            parts
        }
        "rock" => vec![
            part(
                sphere(0.5, 0.7, 7, 4),
                lib.material(texture, white, 1.0, false),
                trs(
                    Vector3::new(0.0, 0.18, 0.0),
                    0.0,
                    Vector3::new(1.0, 1.0, 0.85),
                ),
            ),
            part(
                sphere(0.28, 0.4, 6, 3),
                lib.material(texture, white, 1.0, false),
                trs(Vector3::new(0.35, 0.1, 0.2), 0.7, Vector3::ONE),
            ),
        ],
        "ore" => {
            let mut nugget = lib
                .material("steel", color, 1.0, false)
                .duplicate_resource();
            nugget.set_emission(color.darkened(0.5));
            nugget.set_feature(Feature::EMISSION, true);
            vec![
                part(
                    sphere(0.5, 0.8, 7, 4),
                    lib.material(texture, white, 1.0, false),
                    trs(
                        Vector3::new(0.0, 0.22, 0.0),
                        0.0,
                        Vector3::new(1.0, 1.0, 0.9),
                    ),
                ),
                part(
                    cube(Vector3::new(0.18, 0.18, 0.18)),
                    nugget.clone(),
                    rot(
                        Vector3::new(0.18, 0.5, 0.1),
                        Vector3::new(0.6, 0.8, 0.3),
                        Vector3::ONE,
                    ),
                ),
                part(
                    cube(Vector3::new(0.12, 0.12, 0.12)),
                    nugget,
                    rot(
                        Vector3::new(-0.2, 0.42, -0.15),
                        Vector3::new(0.2, 0.3, 0.9),
                        Vector3::ONE,
                    ),
                ),
            ]
        }
        "crystal" => {
            let mut mat = lib
                .material(texture, white, 1.0, false)
                .duplicate_resource();
            mat.set_emission(color.darkened(0.2));
            mat.set_feature(Feature::EMISSION, true);
            let shard = cylinder(0.0, 0.16, 1.0, 6);
            let tilt = |x: f32, z: f32, pos: Vector3, s: f32| Part {
                mesh: shard.clone(),
                material: mat.clone(),
                local: rot(pos, Vector3::new(x, 0.0, z), Vector3::splat(s)),
            };
            vec![
                tilt(0.0, 0.0, Vector3::new(0.0, 0.5, 0.0), 1.1),
                tilt(0.5, 0.3, Vector3::new(0.18, 0.35, 0.12), 0.75),
                tilt(-0.4, -0.5, Vector3::new(-0.2, 0.32, -0.05), 0.7),
                part(
                    sphere(0.35, 0.3, 7, 3),
                    lib.material("rocky", white, 1.0, false),
                    at(Vector3::new(0.0, 0.05, 0.0)),
                ),
            ]
        }
        _ => vec![part(
            cube(Vector3::new(0.6, 0.6, 0.6)),
            lib.material(texture, color, 1.0, false),
            at(Vector3::new(0.0, 0.3, 0.0)),
        )],
    }
}

// ---------------------------------------------------------------- actors

/// A character model. `body` reflects armor, `weapon_pivot` swings when
/// attacking, `spin` nodes rotate (propellers), `wings` flap, `float`
/// lifts flying creatures.
pub struct ActorModel {
    pub root: Gd<Node3D>,
    pub body: Option<Gd<MeshInstance3D>>,
    pub weapon_pivot: Option<Gd<Node3D>>,
    pub height: f32,
    pub legs: Vec<Gd<Node3D>>,
    pub arms: Vec<Gd<Node3D>>,
    pub spin: Vec<Gd<Node3D>>,
    pub wings: Vec<Gd<Node3D>>,
    pub float: f32,
    pub head: Option<Gd<Node3D>>,
}

impl ActorModel {
    fn new(height: f32) -> ActorModel {
        ActorModel {
            root: Node3D::new_alloc(),
            body: None,
            weapon_pivot: None,
            height,
            legs: Vec::new(),
            arms: Vec::new(),
            spin: Vec::new(),
            wings: Vec::new(),
            float: 0.0,
            head: None,
        }
    }
    fn add(&mut self, mi: Gd<MeshInstance3D>) -> Gd<MeshInstance3D> {
        self.root.add_child(&mi);
        mi
    }
}

fn pivot(parent: &mut Gd<Node3D>, pos: Vector3) -> Gd<Node3D> {
    let mut p = Node3D::new_alloc();
    p.set_position(pos);
    parent.add_child(&p);
    p
}

pub const SKINS: [&str; 5] = ["#f2d0b4", "#e2ac8c", "#c8966a", "#9a6a42", "#5e3c24"];
pub const HAIRS: [&str; 7] = [
    "#18120e", "#4a2e1a", "#7a3a18", "#d8b060", "#a8361a", "#8a8a88", "#e8e6e0",
];
pub const EYES: [&str; 5] = ["#4a2e1a", "#3a6ac0", "#3a8a4a", "#6a7a8a", "#c08a2a"];

/// Everything that shapes a humanoid.
#[derive(Clone)]
pub struct HumanStyle {
    pub look: Look,
    pub skin_tex: String,
    pub body_tex: String,
    pub body: Color,
    pub legs: Color,
    pub accent: Color,
    pub outfit: String,
    pub weapon: String,
    pub weapon_color: Color,
    pub offhand: String,
    pub offhand_color: Color,
    pub helm: Option<Color>,
    pub monster: bool,
    pub skeletal: bool,
    pub horns: bool,
    pub wings: bool,
    pub cape: Option<Color>,
}

impl Default for HumanStyle {
    fn default() -> Self {
        HumanStyle {
            look: Look::default(),
            skin_tex: "skin".into(),
            body_tex: "cloth".into(),
            body: Color::from_rgb(0.6, 0.45, 0.3),
            legs: Color::from_rgb(0.35, 0.28, 0.22),
            accent: Color::from_rgb(0.72, 0.51, 0.23),
            outfit: "peasant".into(),
            weapon: String::new(),
            weapon_color: Color::from_rgb(0.85, 0.86, 0.9),
            offhand: String::new(),
            offhand_color: Color::WHITE,
            helm: None,
            monster: false,
            skeletal: false,
            horns: false,
            wings: false,
            cape: None,
        }
    }
}

/// Builds the weapon mesh(es) under a pivot; the blade points along +Z.
pub fn build_weapon(pivot: &mut Gd<Node3D>, lib: &ContentLibrary, kind: &str, color: Color) {
    for mut c in pivot.get_children().iter_shared() {
        c.queue_free();
    }
    if kind.is_empty() {
        return;
    }
    let metal = lib.material("steel", color, 1.0, false);
    let wood = lib.material("wood", Color::from_rgb(0.75, 0.6, 0.45), 1.0, false);
    let grip = lib.material("leather", Color::from_rgb(0.5, 0.35, 0.25), 1.0, false);
    let flat =
        |pos: Vector3| Transform3D::new(Basis::from_axis_angle(Vector3::RIGHT, FRAC_PI_2), pos);
    let mut add = |mesh: Gd<Mesh>, mat: &Gd<StandardMaterial3D>, tf: Transform3D| {
        pivot.add_child(&instance(&mesh, mat, tf));
    };
    match kind {
        "axe" => {
            add(
                cylinder(0.025, 0.03, 0.85, 8),
                &wood,
                flat(Vector3::new(0.0, 0.0, 0.35)),
            );
            add(
                cube(Vector3::new(0.04, 0.26, 0.2)),
                &metal,
                at(Vector3::new(0.0, 0.1, 0.68)),
            );
        }
        "mace" => {
            add(
                cylinder(0.025, 0.03, 0.7, 8),
                &wood,
                flat(Vector3::new(0.0, 0.0, 0.3)),
            );
            add(
                sphere(0.1, 0.2, 10, 6),
                &metal,
                at(Vector3::new(0.0, 0.0, 0.66)),
            );
            for i in 0..6 {
                let a = i as f32 / 6.0 * TAU;
                add(
                    cylinder(0.0, 0.035, 0.1, 6),
                    &metal,
                    rot(
                        Vector3::new(a.cos() * 0.1, a.sin() * 0.1, 0.66),
                        Vector3::new(0.0, 0.0, a - FRAC_PI_2),
                        Vector3::ONE,
                    ),
                );
            }
        }
        "dagger" => {
            add(
                cylinder(0.02, 0.025, 0.12, 8),
                &grip,
                flat(Vector3::new(0.0, 0.0, 0.04)),
            );
            add(
                cube(Vector3::new(0.1, 0.02, 0.03)),
                &metal,
                at(Vector3::new(0.0, 0.0, 0.11)),
            );
            add(
                cylinder(0.0, 0.035, 0.35, 4),
                &metal,
                flat(Vector3::new(0.0, 0.0, 0.3)),
            );
        }
        "spear" => {
            add(
                cylinder(0.022, 0.025, 1.7, 8),
                &wood,
                flat(Vector3::new(0.0, 0.0, 0.45)),
            );
            add(
                cylinder(0.0, 0.05, 0.3, 4),
                &metal,
                flat(Vector3::new(0.0, 0.0, 1.42)),
            );
        }
        "staff" | "wand" => {
            let len = if kind == "staff" { 1.5 } else { 0.5 };
            add(
                cylinder(0.022, 0.03, len, 8),
                &wood,
                flat(Vector3::new(0.0, 0.0, len * 0.3)),
            );
            let orb = glow_material(color, 2.5);
            add(
                sphere(0.07, 0.14, 12, 6),
                &orb,
                at(Vector3::new(0.0, 0.0, len * 0.8)),
            );
        }
        "hammer" => {
            add(
                cylinder(0.025, 0.03, 0.55, 8),
                &wood,
                flat(Vector3::new(0.0, 0.0, 0.22)),
            );
            add(
                cube(Vector3::new(0.1, 0.1, 0.22)),
                &metal,
                rot(
                    Vector3::new(0.0, 0.0, 0.5),
                    Vector3::new(0.0, FRAC_PI_2, 0.0),
                    Vector3::ONE,
                ),
            );
        }
        _ => {
            // Sword: grip, crossguard, long tapered blade.
            add(
                cylinder(0.022, 0.025, 0.18, 8),
                &grip,
                flat(Vector3::new(0.0, 0.0, 0.04)),
            );
            add(
                sphere(0.035, 0.07, 8, 4),
                &metal,
                at(Vector3::new(0.0, 0.0, -0.06)),
            );
            add(
                cube(Vector3::new(0.24, 0.035, 0.04)),
                &metal,
                at(Vector3::new(0.0, 0.0, 0.15)),
            );
            add(
                cube(Vector3::new(0.055, 0.012, 0.75)),
                &metal,
                at(Vector3::new(0.0, 0.0, 0.53)),
            );
            add(
                cylinder(0.0, 0.04, 0.12, 4),
                &metal,
                rot(
                    Vector3::new(0.0, 0.0, 0.96),
                    Vector3::new(FRAC_PI_2, 0.0, 0.0),
                    Vector3::new(1.0, 1.0, 0.25),
                ),
            );
        }
    }
}

pub fn humanoid(lib: &ContentLibrary, st: &HumanStyle) -> ActorModel {
    let l = &st.look;
    let mut m = ActorModel::new(1.9);
    let build = [0.88, 1.0, 1.16][l.build.clamp(0, 2) as usize];
    let skin = if st.monster {
        st.body
    } else {
        parse_color(SKINS[l.skin.rem_euclid(5) as usize])
    };
    let cloth = lib.material(&st.body_tex, st.body, 1.0, false);
    let legs_mat = lib.material(
        if st.monster { &st.body_tex } else { "cloth" },
        st.legs,
        1.0,
        false,
    );
    let leather = lib.material("leather", Color::from_rgb(0.55, 0.42, 0.32), 1.0, false);
    let trim = lib.material("steel", st.accent, 1.0, false);
    let thin: f32 = if st.skeletal { 0.45 } else { 1.0 };
    let female = l.female && !st.monster;

    // Torso: tapered chest, belt, hips.
    let chest_w = if female { 0.27 } else { 0.31 } * build;
    let body = m.add(instance(
        &cylinder(
            chest_w * thin.max(0.6),
            0.22 * build * thin.max(0.6),
            0.58,
            16,
        ),
        &cloth,
        trs(
            Vector3::new(0.0, 1.04, 0.0),
            0.0,
            Vector3::new(1.0, 1.0, 0.68),
        ),
    ));
    m.body = Some(body);
    if female {
        m.add(instance(
            &sphere(0.25 * build, 0.36, 16, 8),
            &cloth,
            trs(
                Vector3::new(0.0, 1.13, 0.05),
                0.0,
                Vector3::new(1.0, 0.7, 0.7),
            ),
        ));
    }
    m.add(instance(
        &cylinder(0.23 * build, 0.24 * build, 0.09, 16),
        &leather,
        trs(
            Vector3::new(0.0, 0.79, 0.0),
            0.0,
            Vector3::new(1.0, 1.0, 0.75),
        ),
    ));
    if !st.monster {
        m.add(instance(
            &cube(Vector3::new(0.09, 0.07, 0.03)),
            &trim,
            at(Vector3::new(0.0, 0.79, 0.175)),
        ));
    }
    if st.skeletal {
        for y in [0.92, 1.03, 1.14] {
            m.add(instance(
                &torus(0.18, 0.22),
                &cloth,
                trs(Vector3::new(0.0, y, 0.0), 0.0, Vector3::new(1.0, 0.5, 0.7)),
            ));
        }
    }
    // Outfit pieces.
    match st.outfit.as_str() {
        "robe" | "priest" | "dress" | "royal" | "robed" => {
            let skirt = lib.material(
                if st.outfit == "dress" {
                    "cloth"
                } else {
                    &st.body_tex
                },
                st.body.darkened(0.08),
                1.0,
                false,
            );
            m.add(instance(
                &cylinder(0.22 * build, 0.36 * build, 0.78, 18),
                &skirt,
                at(Vector3::new(0.0, 0.42, 0.0)),
            ));
            if st.outfit == "priest" {
                m.add(instance(
                    &cube(Vector3::new(0.16, 0.9, 0.03)),
                    &lib.material("cloth", st.accent, 1.0, false),
                    at(Vector3::new(0.0, 0.75, 0.2)),
                ));
            }
        }
        "apron" | "smith" => {
            let apron = lib.material(
                "leather",
                if st.outfit == "smith" {
                    Color::from_rgb(0.35, 0.25, 0.18)
                } else {
                    Color::from_rgb(0.92, 0.9, 0.85)
                },
                1.0,
                false,
            );
            m.add(instance(
                &cube(Vector3::new(0.36, 0.62, 0.03)),
                &apron,
                at(Vector3::new(0.0, 0.82, 0.2)),
            ));
        }
        "guard" | "plate" => {
            let plate = lib.material("steel", Color::from_rgb(0.75, 0.77, 0.82), 1.0, false);
            for side in [-1.0, 1.0] {
                m.add(instance(
                    &sphere(0.15, 0.2, 12, 6),
                    &plate,
                    at(Vector3::new(side * 0.33, 1.27, 0.0)),
                ));
            }
            m.add(instance(
                &cube(Vector3::new(0.34, 0.5, 0.05)),
                &lib.material("cloth", st.accent, 1.0, false),
                at(Vector3::new(0.0, 0.95, 0.19)),
            ));
        }
        "merchant" => {
            m.add(instance(
                &cylinder(0.17, 0.2, 0.18, 14),
                &lib.material("cloth", st.accent, 1.0, false),
                at(Vector3::new(0.0, 1.78, -0.01)),
            ));
            m.add(instance(
                &cylinder(0.27, 0.27, 0.02, 16),
                &lib.material("cloth", st.accent, 1.0, false),
                at(Vector3::new(0.0, 1.69, -0.01)),
            ));
        }
        "cloak" | "bandit" => {
            m.add(instance(
                &sphere(0.21, 0.32, 14, 7),
                &lib.material("cloth", st.accent, 1.0, false),
                trs(
                    Vector3::new(0.0, 1.6, -0.03),
                    0.0,
                    Vector3::new(1.0, 1.0, 1.05),
                ),
            ));
        }
        _ => {}
    }

    // Head and neck.
    let mut head = pivot(&mut m.root, Vector3::new(0.0, 1.53, 0.0));
    let head_mat = lib.material(
        if st.monster { &st.skin_tex } else { "skin" },
        skin,
        1.0,
        false,
    );
    m.root.add_child(&instance(
        &cylinder(0.065, 0.08, 0.16, 10),
        &head_mat,
        at(Vector3::new(0.0, 1.35, 0.0)),
    ));
    head.add_child(&instance(
        &sphere(0.175, 0.37, 18, 10),
        &head_mat,
        trs(Vector3::ZERO, 0.0, Vector3::new(0.9, 1.0, 0.95)),
    ));
    head.add_child(&instance(
        &sphere(0.035, 0.07, 8, 4),
        &head_mat,
        at(Vector3::new(0.0, -0.02, 0.17)),
    ));
    for side in [-1.0, 1.0] {
        head.add_child(&instance(
            &sphere(0.035, 0.06, 8, 4),
            &head_mat,
            at(Vector3::new(side * 0.16, 0.0, 0.0)),
        ));
    }
    let eye_color = if st.monster {
        if st.skeletal {
            Color::from_rgb(1.0, 0.4, 0.2)
        } else {
            Color::from_rgb(0.9, 0.8, 0.2)
        }
    } else {
        parse_color(EYES[l.eyes.rem_euclid(5) as usize])
    };
    let white = lib.material("cloth", Color::from_rgb(0.95, 0.95, 0.93), 1.0, false);
    let iris = if st.monster {
        glow_material(eye_color, 2.0)
    } else {
        lib.material("cloth", eye_color, 1.0, false)
    };
    for x in [-0.062, 0.062] {
        if !st.monster {
            head.add_child(&instance(
                &sphere(0.026, 0.04, 10, 5),
                &white,
                at(Vector3::new(x, 0.02, 0.145)),
            ));
        }
        head.add_child(&instance(
            &sphere(0.015, 0.025, 8, 4),
            &iris,
            at(Vector3::new(x, 0.02, 0.166)),
        ));
        head.add_child(&instance(
            &cube(Vector3::new(0.06, 0.014, 0.02)),
            &leather,
            at(Vector3::new(x, 0.065, 0.155)),
        ));
    }
    if !st.monster {
        let lips = lib.material("skin", skin.darkened(0.25), 1.0, false);
        head.add_child(&instance(
            &cube(Vector3::new(0.06, 0.012, 0.015)),
            &lips,
            at(Vector3::new(0.0, -0.075, 0.155)),
        ));
    }
    // Hair and beard.
    if !st.monster
        && st.helm.is_none()
        && !matches!(st.outfit.as_str(), "cloak" | "bandit" | "merchant")
    {
        let hair = lib.material(
            "fur",
            parse_color(HAIRS[l.hair_color.rem_euclid(7) as usize]),
            2.0,
            false,
        );
        let style = if female && l.hair == 0 {
            2
        } else {
            l.hair.rem_euclid(6)
        };
        match style {
            0 => {}
            1 => {
                head.add_child(&instance(
                    &sphere(0.185, 0.25, 16, 8),
                    &hair,
                    trs(
                        Vector3::new(0.0, 0.07, -0.015),
                        0.0,
                        Vector3::new(0.95, 1.0, 1.0),
                    ),
                ));
            }
            2 => {
                head.add_child(&instance(
                    &sphere(0.19, 0.27, 16, 8),
                    &hair,
                    at(Vector3::new(0.0, 0.07, -0.02)),
                ));
                head.add_child(&instance(
                    &cylinder(0.15, 0.19, 0.42, 14),
                    &hair,
                    trs(
                        Vector3::new(0.0, -0.14, -0.07),
                        0.0,
                        Vector3::new(1.0, 1.0, 0.6),
                    ),
                ));
            }
            3 => {
                head.add_child(&instance(
                    &sphere(0.185, 0.25, 16, 8),
                    &hair,
                    at(Vector3::new(0.0, 0.07, -0.015)),
                ));
                head.add_child(&instance(
                    &capsule(0.05, 0.4),
                    &hair,
                    rot(
                        Vector3::new(0.0, -0.1, -0.2),
                        Vector3::new(0.35, 0.0, 0.0),
                        Vector3::ONE,
                    ),
                ));
            }
            4 => {
                head.add_child(&instance(
                    &cube(Vector3::new(0.05, 0.12, 0.34)),
                    &hair,
                    at(Vector3::new(0.0, 0.17, -0.02)),
                ));
            }
            _ => {
                head.add_child(&instance(
                    &sphere(0.185, 0.25, 16, 8),
                    &hair,
                    at(Vector3::new(0.0, 0.07, -0.015)),
                ));
                for side in [-1.0, 1.0] {
                    head.add_child(&instance(
                        &capsule(0.04, 0.45),
                        &hair,
                        at(Vector3::new(side * 0.13, -0.18, -0.05)),
                    ));
                }
            }
        }
        if !female && l.beard > 0 {
            let len = [0.0, 0.05, 0.14, 0.26][l.beard.clamp(0, 3) as usize];
            head.add_child(&instance(
                &sphere(0.13, 0.12 + len, 14, 7),
                &hair,
                trs(
                    Vector3::new(0.0, -0.1 - len * 0.35, 0.09),
                    0.0,
                    Vector3::new(1.0, 1.0, 0.7),
                ),
            ));
        }
    }
    if let Some(c) = st.helm {
        let metal = lib.material("steel", c, 1.0, false);
        head.add_child(&instance(
            &sphere(0.2, 0.26, 16, 8),
            &metal,
            at(Vector3::new(0.0, 0.06, -0.01)),
        ));
        head.add_child(&instance(
            &cube(Vector3::new(0.03, 0.14, 0.03)),
            &metal,
            at(Vector3::new(0.0, 0.0, 0.19)),
        ));
        head.add_child(&instance(
            &cylinder(0.205, 0.205, 0.04, 16),
            &trim,
            at(Vector3::new(0.0, 0.04, -0.01)),
        ));
    }
    if st.outfit == "royal" {
        let gold = glow_material(Color::from_rgb(1.0, 0.8, 0.25), 0.4);
        head.add_child(&instance(
            &cylinder(0.15, 0.14, 0.09, 16),
            &gold,
            at(Vector3::new(0.0, 0.2, 0.0)),
        ));
        for i in 0..6 {
            let a = i as f32 / 6.0 * TAU;
            head.add_child(&instance(
                &cylinder(0.0, 0.025, 0.08, 6),
                &gold,
                at(Vector3::new(a.cos() * 0.14, 0.27, a.sin() * 0.14)),
            ));
        }
    }
    if st.horns {
        let horn = lib.material("bone", Color::from_rgb(0.3, 0.25, 0.2), 1.0, false);
        for side in [-1.0, 1.0] {
            head.add_child(&instance(
                &cylinder(0.0, 0.05, 0.2, 8),
                &horn,
                rot(
                    Vector3::new(side * 0.11, 0.18, 0.0),
                    Vector3::new(0.0, 0.0, -side * 0.5),
                    Vector3::ONE,
                ),
            ));
        }
    }
    m.head = Some(head);

    // Legs with boots.
    for side in [-1.0f32, 1.0] {
        let mut leg = pivot(&mut m.root, Vector3::new(side * 0.12 * build, 0.75, 0.0));
        leg.add_child(&instance(
            &capsule(0.095 * thin * build, 0.56),
            &legs_mat,
            at(Vector3::new(0.0, -0.28, 0.0)),
        ));
        let boot = if st.skeletal {
            cloth.clone()
        } else {
            leather.clone()
        };
        leg.add_child(&instance(
            &capsule(0.1 * thin.max(0.8), 0.24),
            &boot,
            at(Vector3::new(0.0, -0.58, 0.02)),
        ));
        leg.add_child(&instance(
            &sphere(0.1, 0.13, 12, 6),
            &boot,
            trs(
                Vector3::new(0.0, -0.68, 0.07),
                0.0,
                Vector3::new(0.9, 1.0, 1.6),
            ),
        ));
        m.legs.push(leg);
    }
    // Arms: shoulder pivot so they swing while walking.
    for side in [-1.0f32, 1.0] {
        let mut arm = pivot(
            &mut m.root,
            Vector3::new(side * (chest_w + 0.06), 1.25, 0.0),
        );
        arm.add_child(&instance(
            &capsule(0.075 * thin * build, 0.34),
            &cloth,
            at(Vector3::new(0.0, -0.15, 0.0)),
        ));
        arm.add_child(&instance(
            &capsule(0.065 * thin * build, 0.3),
            if st.outfit == "smith" {
                &head_mat
            } else {
                &cloth
            },
            at(Vector3::new(0.0, -0.4, 0.02)),
        ));
        arm.add_child(&instance(
            &sphere(0.065, 0.12, 10, 5),
            &head_mat,
            at(Vector3::new(0.0, -0.58, 0.03)),
        ));
        if side > 0.0 {
            let mut p = pivot(&mut arm, Vector3::new(0.0, -0.58, 0.05));
            build_weapon(&mut p, lib, &st.weapon, st.weapon_color);
            m.weapon_pivot = Some(p);
        } else if !st.offhand.is_empty() {
            let mut oh = pivot(&mut arm, Vector3::new(-0.06, -0.5, 0.05));
            build_offhand(&mut oh, lib, &st.offhand, st.offhand_color);
        }
        m.arms.push(arm);
    }
    // Cape.
    if let Some(cape_color) = st.cape {
        let mut cape = SurfaceTool::new_gd();
        cape.begin(godot::classes::mesh::PrimitiveType::TRIANGLES);
        let pts = [
            Vector3::new(-0.24, 1.3, -0.17),
            Vector3::new(0.24, 1.3, -0.17),
            Vector3::new(-0.36, 0.22, -0.36),
            Vector3::new(0.36, 0.22, -0.36),
            Vector3::new(0.0, 0.7, -0.42),
        ];
        for (index, uv) in [
            (0, (0.0, 0.0)),
            (4, (0.5, 0.5)),
            (1, (1.0, 0.0)),
            (0, (0.0, 0.0)),
            (2, (0.0, 1.0)),
            (4, (0.5, 0.5)),
            (1, (1.0, 0.0)),
            (4, (0.5, 0.5)),
            (3, (1.0, 1.0)),
            (2, (0.0, 1.0)),
            (3, (1.0, 1.0)),
            (4, (0.5, 0.5)),
        ] {
            cape.set_uv(Vector2::new(uv.0, uv.1));
            cape.add_vertex(pts[index]);
        }
        cape.generate_normals();
        if let Some(mesh) = cape.commit() {
            let mut mat = lib
                .material("cloak", cape_color, 1.0, false)
                .duplicate_resource();
            mat.set_cull_mode(CullMode::DISABLED);
            m.root
                .add_child(&instance(&mesh.upcast(), &mat, at(Vector3::ZERO)));
        }
    }
    if st.wings {
        let wing = lib
            .material("skin", st.body.darkened(0.3), 1.0, false)
            .duplicate_resource();
        let mut wing = wing;
        wing.set_cull_mode(CullMode::DISABLED);
        for side in [-1.0f32, 1.0] {
            let mut w = pivot(&mut m.root, Vector3::new(side * 0.12, 1.2, -0.15));
            w.add_child(&instance(
                &blade(0.6, 0.45),
                &wing,
                rot(
                    Vector3::ZERO,
                    Vector3::new(0.0, if side > 0.0 { 0.3 } else { PI - 0.3 }, 0.3),
                    Vector3::ONE,
                ),
            ));
            m.wings.push(w);
        }
    }
    m
}

pub fn build_offhand(p: &mut Gd<Node3D>, lib: &ContentLibrary, kind: &str, color: Color) {
    match kind {
        "tome" => {
            p.add_child(&instance(
                &cube(Vector3::new(0.05, 0.22, 0.17)),
                &lib.material("leather", color, 1.0, false),
                at(Vector3::ZERO),
            ));
        }
        _ => {
            let metal = lib.material("steel", color, 1.0, false);
            p.add_child(&instance(
                &cylinder(0.24, 0.24, 0.05, 18),
                &metal,
                rot(
                    Vector3::new(-0.05, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, FRAC_PI_2),
                    Vector3::ONE,
                ),
            ));
            p.add_child(&instance(
                &sphere(0.06, 0.08, 10, 5),
                &lib.material("steel", Color::from_rgb(0.8, 0.65, 0.3), 1.0, false),
                at(Vector3::new(-0.08, 0.0, 0.0)),
            ));
        }
    }
}

fn quadruped(lib: &ContentLibrary, texture: &str, color: Color, kind: &str) -> ActorModel {
    let (bw, bl, bh, leg_len, head_r, height) = match kind {
        "bear" => (0.42, 1.6, 0.85, 0.42, 0.3, 1.4),
        "boar" => (0.33, 1.6, 0.6, 0.3, 0.25, 0.95),
        "cat" => (0.18, 1.8, 0.42, 0.25, 0.16, 0.65),
        _ => (0.3, 1.9, 0.58, 0.38, 0.23, 1.1),
    };
    let mut m = ActorModel::new(height);
    let fur = lib.material(texture, color, 1.0, false);
    let body = m.add(instance(
        &sphere(bw, bw * 1.6, 16, 8),
        &fur,
        trs(Vector3::new(0.0, bh, 0.0), 0.0, Vector3::new(1.0, 1.0, bl)),
    ));
    m.body = Some(body);
    let neck_z = bw * bl * 0.9;
    let mut head = pivot(&mut m.root, Vector3::new(0.0, bh + 0.2, neck_z));
    head.add_child(&instance(
        &sphere(head_r, head_r * 1.7, 16, 8),
        &fur,
        at(Vector3::ZERO),
    ));
    let snout = if kind == "bear" {
        0.85
    } else if kind == "cat" {
        0.6
    } else {
        1.8
    };
    head.add_child(&instance(
        &sphere(head_r * 0.55, head_r * 0.8, 14, 7),
        &fur,
        trs(
            Vector3::new(0.0, -0.05, head_r * 1.0),
            0.0,
            Vector3::new(0.8, 1.0, snout),
        ),
    ));
    let nose = lib.material("cloth", Color::from_rgb(0.05, 0.04, 0.04), 1.0, false);
    head.add_child(&instance(
        &sphere(0.04, 0.05, 8, 4),
        &nose,
        at(Vector3::new(0.0, -0.02, head_r * (1.0 + snout * 0.5))),
    ));
    let eye = glow_material(
        if kind == "cat" {
            Color::from_rgb(0.5, 1.0, 0.3)
        } else {
            Color::from_rgb(1.0, 0.8, 0.25)
        },
        0.8,
    );
    for x in [-1.0f32, 1.0] {
        head.add_child(&instance(
            &sphere(0.03, 0.05, 8, 4),
            &eye,
            at(Vector3::new(x * head_r * 0.45, head_r * 0.25, head_r * 0.8)),
        ));
        match kind {
            "bear" => head.add_child(&instance(
                &sphere(0.07, 0.08, 10, 5),
                &fur,
                at(Vector3::new(x * head_r * 0.7, head_r * 0.85, 0.0)),
            )),
            "boar" => {
                head.add_child(&instance(
                    &cylinder(0.0, 0.03, 0.14, 6),
                    &lib.material("bone", Color::WHITE, 1.0, false),
                    rot(
                        Vector3::new(x * 0.1, -0.08, head_r * 1.5),
                        Vector3::new(-0.6, 0.0, x * 0.4),
                        Vector3::ONE,
                    ),
                ));
                head.add_child(&instance(
                    &cylinder(0.0, 0.06, 0.12, 6),
                    &fur,
                    at(Vector3::new(x * head_r * 0.6, head_r * 0.9, 0.0)),
                ));
            }
            _ => head.add_child(&instance(
                &cylinder(0.0, 0.07, 0.2, 6),
                &fur,
                at(Vector3::new(x * head_r * 0.55, head_r * 1.05, -0.02)),
            )),
        }
    }
    m.head = Some(head);
    for (x, z) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let mut leg = pivot(
            &mut m.root,
            Vector3::new(x * bw * 0.55, bh - 0.1, z * bw * bl * 0.55),
        );
        leg.add_child(&instance(
            &capsule(bw * 0.24, leg_len + 0.1),
            &fur,
            at(Vector3::new(0.0, -leg_len * 0.5, 0.0)),
        ));
        m.legs.push(leg);
    }
    let tail_len = match kind {
        "bear" => 0.15,
        "boar" => 0.25,
        "cat" => 0.6,
        _ => 0.55,
    };
    m.root.add_child(&instance(
        &capsule(0.06, tail_len),
        &fur,
        rot(
            Vector3::new(0.0, bh + 0.05, -bw * bl * 0.95),
            Vector3::new(if kind == "cat" { -0.4 } else { 1.0 }, 0.0, 0.0),
            Vector3::ONE,
        ),
    ));
    m
}

fn spider(lib: &ContentLibrary, texture: &str, color: Color) -> ActorModel {
    let mut m = ActorModel::new(0.9);
    let skin = lib.material(texture, color, 1.0, false);
    let body = m.add(instance(
        &sphere(0.36, 0.6, 16, 8),
        &skin,
        trs(
            Vector3::new(0.0, 0.5, -0.32),
            0.0,
            Vector3::new(1.0, 0.9, 1.2),
        ),
    ));
    m.body = Some(body);
    m.add(instance(
        &sphere(0.22, 0.36, 14, 7),
        &skin,
        at(Vector3::new(0.0, 0.42, 0.18)),
    ));
    let eye = glow_material(Color::from_rgb(1.0, 0.15, 0.1), 2.5);
    for (x, y) in [
        (-0.08, 0.5),
        (0.08, 0.5),
        (-0.13, 0.46),
        (0.13, 0.46),
        (-0.04, 0.55),
        (0.04, 0.55),
    ] {
        m.add(instance(
            &sphere(0.025, 0.05, 8, 4),
            &eye,
            at(Vector3::new(x, y, 0.37)),
        ));
    }
    for side in [-1.0f32, 1.0] {
        for i in 0..4 {
            let z = 0.25 - i as f32 * 0.14;
            let mut leg = pivot(&mut m.root, Vector3::new(side * 0.15, 0.48, z));
            leg.add_child(&instance(
                &capsule(0.035, 0.5),
                &skin,
                rot(
                    Vector3::new(side * 0.22, 0.1, 0.0),
                    Vector3::new(0.0, 0.0, side * 1.1),
                    Vector3::ONE,
                ),
            ));
            leg.add_child(&instance(
                &capsule(0.03, 0.55),
                &skin,
                rot(
                    Vector3::new(side * 0.47, -0.15, 0.0),
                    Vector3::new(0.0, 0.0, -side * 0.35),
                    Vector3::ONE,
                ),
            ));
            m.legs.push(leg);
        }
    }
    m
}

fn winged_flyer(lib: &ContentLibrary, texture: &str, color: Color, bird: bool) -> ActorModel {
    let mut m = ActorModel::new(0.9);
    m.float = 1.1;
    let skin = lib.material(texture, color, 1.0, false);
    let body = m.add(instance(
        &sphere(0.16, 0.34, 14, 7),
        &skin,
        trs(
            Vector3::new(0.0, 0.0, 0.0),
            0.0,
            Vector3::new(1.0, 1.0, if bird { 1.6 } else { 1.1 }),
        ),
    ));
    m.body = Some(body);
    m.add(instance(
        &sphere(0.11, 0.2, 12, 6),
        &skin,
        at(Vector3::new(0.0, 0.1, 0.2)),
    ));
    let eye = glow_material(
        if bird {
            Color::from_rgb(0.9, 0.9, 0.9)
        } else {
            Color::from_rgb(1.0, 0.3, 0.2)
        },
        1.5,
    );
    for x in [-0.045, 0.045] {
        m.add(instance(
            &sphere(0.02, 0.035, 8, 4),
            &eye,
            at(Vector3::new(x, 0.13, 0.29)),
        ));
    }
    if bird {
        m.add(instance(
            &cylinder(0.0, 0.04, 0.14, 6),
            &lib.material("bone", Color::from_rgb(0.25, 0.25, 0.2), 1.0, false),
            rot(
                Vector3::new(0.0, 0.08, 0.34),
                Vector3::new(FRAC_PI_2, 0.0, 0.0),
                Vector3::ONE,
            ),
        ));
    } else {
        for x in [-1.0f32, 1.0] {
            m.add(instance(
                &cylinder(0.0, 0.05, 0.14, 6),
                &skin,
                at(Vector3::new(x * 0.07, 0.22, 0.18)),
            ));
        }
    }
    let mut wing_mat = lib
        .material(texture, color.darkened(0.2), 1.0, false)
        .duplicate_resource();
    wing_mat.set_cull_mode(CullMode::DISABLED);
    for side in [-1.0f32, 1.0] {
        let mut w = pivot(&mut m.root, Vector3::new(side * 0.1, 0.05, 0.0));
        w.add_child(&instance(
            &blade(if bird { 0.55 } else { 0.6 }, 0.4),
            &wing_mat,
            rot(
                Vector3::ZERO,
                Vector3::new(0.0, if side > 0.0 { 0.0 } else { PI }, 0.0),
                Vector3::ONE,
            ),
        ));
        m.wings.push(w);
    }
    m
}

fn golem(lib: &ContentLibrary, texture: &str, color: Color) -> ActorModel {
    let mut m = ActorModel::new(1.9);
    let stone = lib.material(texture, color, 1.0, false);
    let core = glow_material(Color::from_rgb(0.4, 0.8, 1.0), 2.0);
    let body = m.add(instance(
        &sphere(0.45, 0.8, 7, 4),
        &stone,
        trs(
            Vector3::new(0.0, 1.1, 0.0),
            0.0,
            Vector3::new(1.2, 1.0, 0.9),
        ),
    ));
    m.body = Some(body);
    m.add(instance(
        &sphere(0.1, 0.18, 10, 5),
        &core,
        at(Vector3::new(0.0, 1.15, 0.4)),
    ));
    let mut head = pivot(&mut m.root, Vector3::new(0.0, 1.62, 0.05));
    head.add_child(&instance(
        &cube(Vector3::new(0.32, 0.26, 0.3)),
        &stone,
        at(Vector3::ZERO),
    ));
    for x in [-0.07, 0.07] {
        head.add_child(&instance(
            &sphere(0.03, 0.05, 8, 4),
            &core,
            at(Vector3::new(x, 0.02, 0.15)),
        ));
    }
    m.head = Some(head);
    for side in [-1.0f32, 1.0] {
        let mut arm = pivot(&mut m.root, Vector3::new(side * 0.62, 1.35, 0.0));
        arm.add_child(&instance(
            &sphere(0.22, 0.4, 7, 4),
            &stone,
            at(Vector3::new(0.0, -0.1, 0.0)),
        ));
        arm.add_child(&instance(
            &sphere(0.2, 0.5, 7, 4),
            &stone,
            at(Vector3::new(0.0, -0.5, 0.05)),
        ));
        arm.add_child(&instance(
            &sphere(0.24, 0.36, 7, 4),
            &stone,
            at(Vector3::new(0.0, -0.85, 0.1)),
        ));
        m.arms.push(arm);
        let mut leg = pivot(&mut m.root, Vector3::new(side * 0.25, 0.7, 0.0));
        leg.add_child(&instance(
            &sphere(0.2, 0.7, 7, 4),
            &stone,
            at(Vector3::new(0.0, -0.35, 0.0)),
        ));
        m.legs.push(leg);
    }
    m
}

fn wisp(lib: &ContentLibrary, color: Color) -> ActorModel {
    let mut m = ActorModel::new(1.2);
    m.float = 0.9;
    let core = glow_material(color, 3.0);
    let body = m.add(instance(
        &sphere(0.18, 0.36, 16, 8),
        &core,
        at(Vector3::ZERO),
    ));
    m.body = Some(body);
    m.add(instance(
        &sphere(0.32, 0.64, 16, 8),
        &ghost_material(Color::from_rgba(color.r, color.g, color.b, 0.25)),
        at(Vector3::ZERO),
    ));
    let mut ring = pivot(&mut m.root, Vector3::ZERO);
    let shard = lib.material("crystal", color, 1.0, false);
    for i in 0..5 {
        let a = i as f32 / 5.0 * TAU;
        ring.add_child(&instance(
            &cylinder(0.0, 0.06, 0.25, 5),
            &shard,
            rot(
                Vector3::new(a.cos() * 0.42, (a * 2.0).sin() * 0.1, a.sin() * 0.42),
                Vector3::new(0.3, a, 0.0),
                Vector3::ONE,
            ),
        ));
    }
    m.spin.push(ring);
    let mut l = light(color, 1.2, 4.0);
    l.set_position(Vector3::ZERO);
    m.root.add_child(&l);
    m
}

fn drake(lib: &ContentLibrary, texture: &str, color: Color) -> ActorModel {
    let mut m = ActorModel::new(1.6);
    let skin = lib.material(texture, color, 1.0, false);
    let belly = lib.material(texture, color.lightened(0.35), 1.0, false);
    let horn = lib.material("bone", Color::from_rgb(0.9, 0.85, 0.7), 1.0, false);
    let body = m.add(instance(
        &sphere(0.38, 0.62, 16, 8),
        &skin,
        trs(
            Vector3::new(0.0, 0.75, 0.0),
            0.0,
            Vector3::new(1.0, 1.0, 1.9),
        ),
    ));
    m.body = Some(body);
    m.add(instance(
        &sphere(0.3, 0.5, 14, 7),
        &belly,
        trs(
            Vector3::new(0.0, 0.62, 0.05),
            0.0,
            Vector3::new(1.0, 0.8, 1.7),
        ),
    ));
    m.add(instance(
        &capsule(0.13, 0.75),
        &skin,
        rot(
            Vector3::new(0.0, 1.1, 0.75),
            Vector3::new(-0.7, 0.0, 0.0),
            Vector3::ONE,
        ),
    ));
    let mut head = pivot(&mut m.root, Vector3::new(0.0, 1.38, 1.02));
    head.add_child(&instance(
        &sphere(0.17, 0.28, 14, 7),
        &skin,
        trs(Vector3::ZERO, 0.0, Vector3::new(1.0, 1.0, 1.7)),
    ));
    let eye = glow_material(Color::from_rgb(1.0, 0.7, 0.1), 2.0);
    for x in [-1.0f32, 1.0] {
        head.add_child(&instance(
            &sphere(0.03, 0.05, 8, 4),
            &eye,
            at(Vector3::new(x * 0.1, 0.06, 0.15)),
        ));
        head.add_child(&instance(
            &cylinder(0.0, 0.05, 0.3, 6),
            &horn,
            rot(
                Vector3::new(x * 0.08, 0.12, -0.1),
                Vector3::new(-1.0, 0.0, x * 0.3),
                Vector3::ONE,
            ),
        ));
    }
    m.head = Some(head);
    for (x, z) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let mut leg = pivot(&mut m.root, Vector3::new(x * 0.25, 0.6, z * 0.4));
        leg.add_child(&instance(
            &capsule(0.1, 0.55),
            &skin,
            at(Vector3::new(0.0, -0.25, 0.0)),
        ));
        m.legs.push(leg);
    }
    let mut tail = pivot(&mut m.root, Vector3::new(0.0, 0.75, -0.7));
    tail.add_child(&instance(
        &cylinder(0.02, 0.14, 1.0, 10),
        &skin,
        rot(
            Vector3::new(0.0, -0.15, -0.45),
            Vector3::new(-1.3, 0.0, 0.0),
            Vector3::ONE,
        ),
    ));
    let mut wing_mat = lib
        .material("skin", color.darkened(0.35), 1.0, false)
        .duplicate_resource();
    wing_mat.set_cull_mode(CullMode::DISABLED);
    for side in [-1.0f32, 1.0] {
        let mut w = pivot(&mut m.root, Vector3::new(side * 0.25, 1.0, 0.15));
        w.add_child(&instance(
            &blade(1.2, 0.9),
            &wing_mat,
            rot(
                Vector3::ZERO,
                Vector3::new(0.0, if side > 0.0 { 0.2 } else { PI - 0.2 }, 0.0),
                Vector3::ONE,
            ),
        ));
        m.wings.push(w);
    }
    m
}

/// The Vintokryly Pizdokryl: a round, grumpy brass-and-fur creature that
/// flies on two spinning propellers, with stubby legs and a big mouth.
fn vintokryl(lib: &ContentLibrary, color: Color) -> ActorModel {
    let mut m = ActorModel::new(1.2);
    m.float = 1.3;
    let fur = lib.material("fur", Color::from_rgb(0.55, 0.4, 0.3), 1.0, false);
    let brass = lib.material("steel", color, 1.0, false);
    let dark = lib.material("rusted_iron", Color::WHITE, 1.0, false);
    let body = m.add(instance(
        &sphere(0.32, 0.6, 20, 10),
        &fur,
        at(Vector3::ZERO),
    ));
    m.body = Some(body);
    m.add(instance(
        &torus(0.3, 0.36),
        &brass,
        trs(
            Vector3::new(0.0, 0.02, 0.0),
            0.0,
            Vector3::new(1.0, 1.4, 1.0),
        ),
    ));
    // Goggles and a mouth full of opinions.
    let lens = glow_material(Color::from_rgb(0.4, 1.0, 0.6), 1.8);
    for x in [-0.11, 0.11] {
        m.add(instance(
            &cylinder(0.08, 0.08, 0.06, 16),
            &brass,
            rot(
                Vector3::new(x, 0.1, 0.28),
                Vector3::new(FRAC_PI_2, 0.0, 0.0),
                Vector3::ONE,
            ),
        ));
        m.add(instance(
            &sphere(0.06, 0.06, 12, 6),
            &lens,
            at(Vector3::new(x, 0.1, 0.31)),
        ));
    }
    m.add(instance(
        &sphere(0.14, 0.08, 14, 6),
        &lib.material("cloth", Color::from_rgb(0.4, 0.05, 0.08), 1.0, false),
        trs(
            Vector3::new(0.0, -0.1, 0.27),
            0.0,
            Vector3::new(1.0, 1.0, 0.5),
        ),
    ));
    for i in 0..4 {
        m.add(instance(
            &cube(Vector3::new(0.03, 0.03, 0.02)),
            &lib.material("bone", Color::WHITE, 1.0, false),
            at(Vector3::new(-0.06 + i as f32 * 0.04, -0.07, 0.31)),
        ));
    }
    // Propeller masts on each side.
    for side in [-1.0f32, 1.0] {
        m.add(instance(
            &cylinder(0.025, 0.035, 0.42, 8),
            &dark,
            rot(
                Vector3::new(side * 0.36, 0.18, 0.0),
                Vector3::new(0.0, 0.0, side * 1.1),
                Vector3::ONE,
            ),
        ));
        let mut rotor = pivot(&mut m.root, Vector3::new(side * 0.52, 0.32, 0.0));
        rotor.add_child(&instance(
            &cylinder(0.04, 0.04, 0.06, 10),
            &brass,
            at(Vector3::ZERO),
        ));
        for k in 0..3 {
            let a = k as f32 / 3.0 * TAU;
            rotor.add_child(&instance(
                &blade(0.45, 0.12),
                &brass,
                rot(
                    Vector3::new(0.0, 0.03, 0.0),
                    Vector3::new(0.0, a, 0.12),
                    Vector3::ONE,
                ),
            ));
        }
        m.spin.push(rotor);
    }
    // A tail rotor, because why not.
    let mut tail = pivot(&mut m.root, Vector3::new(0.0, 0.05, -0.42));
    tail.add_child(&instance(
        &cylinder(0.02, 0.02, 0.06, 8),
        &brass,
        rot(
            Vector3::ZERO,
            Vector3::new(FRAC_PI_2, 0.0, 0.0),
            Vector3::ONE,
        ),
    ));
    for k in 0..2 {
        tail.add_child(&instance(
            &blade(0.18, 0.06),
            &brass,
            rot(
                Vector3::ZERO,
                Vector3::new(FRAC_PI_2, 0.0, k as f32 * PI),
                Vector3::ONE,
            ),
        ));
    }
    m.spin.push(tail);
    for x in [-0.12f32, 0.12] {
        let mut leg = pivot(&mut m.root, Vector3::new(x, -0.24, 0.0));
        leg.add_child(&instance(
            &capsule(0.04, 0.2),
            &fur,
            at(Vector3::new(0.0, -0.08, 0.0)),
        ));
        leg.add_child(&instance(
            &sphere(0.05, 0.05, 10, 5),
            &brass,
            at(Vector3::new(0.0, -0.19, 0.04)),
        ));
        m.legs.push(leg);
    }
    m
}

fn slime(lib: &ContentLibrary, texture: &str, color: Color) -> ActorModel {
    let mut m = ActorModel::new(0.8);
    let mut mat = lib
        .material(texture, color, 1.0, false)
        .duplicate_resource();
    mat.set_transparency(Transparency::ALPHA_DEPTH_PRE_PASS);
    mat.set_albedo(Color::from_rgba(color.r, color.g, color.b, 0.82));
    mat.set_rim(0.6);
    mat.set_feature(Feature::RIM, true);
    let body = m.add(instance(
        &sphere(0.45, 0.65, 20, 10),
        &mat,
        at(Vector3::new(0.0, 0.3, 0.0)),
    ));
    m.body = Some(body);
    m.add(instance(
        &sphere(0.18, 0.3, 12, 6),
        &glow_material(color.lightened(0.3), 0.6),
        at(Vector3::new(0.05, 0.28, -0.05)),
    ));
    let eye = lib.material("cloth", Color::from_rgb(0.05, 0.1, 0.05), 1.0, false);
    for x in [-0.14, 0.14] {
        m.add(instance(
            &sphere(0.06, 0.12, 10, 5),
            &eye,
            at(Vector3::new(x, 0.42, 0.36)),
        ));
    }
    m
}

/// Monster and NPC models by content model name.
pub fn actor(model: &str, lib: &ContentLibrary, texture: &str, color: Color) -> ActorModel {
    let mut st = HumanStyle {
        monster: true,
        body: color,
        legs: color,
        skin_tex: texture.into(),
        body_tex: texture.into(),
        ..Default::default()
    };
    match model {
        "slime" => slime(lib, texture, color),
        "wolf" | "bear" | "boar" | "cat" => quadruped(lib, texture, color, model),
        "spider" => spider(lib, texture, color),
        "bat" => winged_flyer(lib, texture, color, false),
        "bird" => winged_flyer(lib, texture, color, true),
        "golem" => golem(lib, texture, color),
        "wisp" => wisp(lib, color),
        "drake" => drake(lib, texture, color),
        "vintokryl" => vintokryl(lib, color),
        "bandit" => {
            st.monster = false;
            st.look = Look {
                skin: 2,
                ..Default::default()
            };
            st.body_tex = "leather".into();
            st.outfit = "bandit".into();
            st.accent = Color::from_rgb(0.6, 0.12, 0.1);
            st.weapon = "dagger".into();
            st.cape = Some(Color::from_rgb(0.45, 0.12, 0.1));
            humanoid(lib, &st)
        }
        "robed" => {
            st.monster = false;
            st.look = Look {
                skin: 0,
                ..Default::default()
            };
            st.outfit = "cloak".into();
            st.accent = color.darkened(0.3);
            st.weapon = "staff".into();
            st.weapon_color = Color::from_rgb(1.0, 0.4, 0.1);
            let mut m = humanoid(lib, &st);
            m.root.add_child(&instance(
                &cylinder(0.24, 0.38, 0.8, 18),
                &lib.material("cloth", color, 1.0, false),
                at(Vector3::new(0.0, 0.42, 0.0)),
            ));
            m
        }
        "imp" => {
            st.horns = true;
            st.wings = true;
            humanoid(lib, &st)
        }
        _ => {
            st.skeletal = texture == "bone";
            humanoid(lib, &st)
        }
    }
}

/// A small floating model for an item lying on the ground.
pub fn ground_item(lib: &ContentLibrary, kind: &str, color: Color) -> Gd<MeshInstance3D> {
    let (mesh, tf) = match kind {
        "gold" => (
            cylinder(0.14, 0.14, 0.05, 16),
            Transform3D::new(Basis::from_axis_angle(Vector3::RIGHT, 1.2), Vector3::ZERO),
        ),
        "potion" | "food" => (sphere(0.15, 0.3, 12, 6), at(Vector3::ZERO)),
        "weapon" | "gear" => (
            cube(Vector3::new(0.06, 0.06, 0.6)),
            Transform3D::new(Basis::from_axis_angle(Vector3::RIGHT, 0.5), Vector3::ZERO),
        ),
        "tool" => (cube(Vector3::new(0.3, 0.08, 0.08)), at(Vector3::ZERO)),
        "armor" => (cube(Vector3::new(0.32, 0.36, 0.12)), at(Vector3::ZERO)),
        "tome" => (cube(Vector3::new(0.25, 0.08, 0.32)), at(Vector3::ZERO)),
        "pet" => (sphere(0.16, 0.24, 12, 6), at(Vector3::ZERO)),
        _ => (cube(Vector3::new(0.22, 0.22, 0.22)), at(Vector3::ZERO)),
    };
    let tex = match kind {
        "gold" | "weapon" | "tool" | "armor" | "gear" => "steel",
        _ => "cloth",
    };
    let mut mat = lib.material(tex, color, 1.0, false).duplicate_resource();
    mat.set_emission(color.darkened(0.5));
    mat.set_feature(Feature::EMISSION, true);
    instance(&mesh, &mat, tf)
}
