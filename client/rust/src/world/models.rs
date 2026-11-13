//! Procedural low-poly models built from Godot primitive meshes.

use godot::classes::{
    BoxMesh, CapsuleMesh, CylinderMesh, Material, Mesh, MeshInstance3D, Node3D, PrimitiveMesh,
    SphereMesh, StandardMaterial3D, SurfaceTool,
};
use godot::prelude::*;

use crate::content::ContentLibrary;

pub fn at(pos: Vector3) -> Transform3D {
    Transform3D::new(Basis::IDENTITY, pos)
}

pub fn trs(pos: Vector3, yaw: f32, scale: Vector3) -> Transform3D {
    Transform3D::new(
        Basis::from_axis_angle(Vector3::UP, yaw) * Basis::from_scale(scale),
        pos,
    )
}

fn finish<T: Inherits<PrimitiveMesh> + Inherits<Mesh>>(m: Gd<T>) -> Gd<Mesh> {
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
    surface.commit().map(|m| m.upcast::<Mesh>()).unwrap_or(mesh)
}

pub fn cylinder(top: f32, bottom: f32, height: f32, segments: i32) -> Gd<Mesh> {
    let mut m = CylinderMesh::new_gd();
    m.set_top_radius(top);
    m.set_bottom_radius(bottom);
    m.set_height(height);
    m.set_radial_segments(segments);
    m.set_rings(1);
    finish(m)
}

pub fn sphere(radius: f32, height: f32, segments: i32, rings: i32) -> Gd<Mesh> {
    let mut m = SphereMesh::new_gd();
    m.set_radius(radius);
    m.set_height(height);
    m.set_radial_segments(segments);
    m.set_rings(rings);
    finish(m)
}

pub fn cube(size: Vector3) -> Gd<Mesh> {
    let mut m = BoxMesh::new_gd();
    m.set_size(size);
    finish(m)
}

pub fn capsule(radius: f32, height: f32) -> Gd<Mesh> {
    let mut m = CapsuleMesh::new_gd();
    m.set_radius(radius);
    m.set_height(height);
    m.set_radial_segments(6);
    m.set_rings(2);
    finish(m)
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

/// One mesh of a multi-part resource model, placed relative to the node.
pub struct Part {
    pub mesh: Gd<Mesh>,
    pub material: Gd<StandardMaterial3D>,
    pub local: Transform3D,
}

/// Parts of a gatherable resource node model ("tree", "rock", "ore", "crystal").
pub fn resource_parts(model: &str, texture: &str, color: Color, lib: &ContentLibrary) -> Vec<Part> {
    let white = Color::WHITE;
    match model {
        "tree" => {
            let leaves = lib.material("leaves", Color::from_rgb(0.85, 1.0, 0.85), 1.5, false);
            vec![
                Part {
                    mesh: cylinder(0.11, 0.17, 1.3, 7),
                    material: lib.material(texture, white, 1.0, false),
                    local: at(Vector3::new(0.0, 0.65, 0.0)),
                },
                Part {
                    mesh: sphere(0.75, 1.2, 9, 5),
                    material: leaves.clone(),
                    local: at(Vector3::new(0.0, 1.55, 0.0)),
                },
                Part {
                    mesh: sphere(0.5, 0.9, 8, 4),
                    material: leaves,
                    local: at(Vector3::new(0.12, 2.15, -0.08)),
                },
            ]
        }
        "rock" => vec![Part {
            mesh: sphere(0.5, 0.7, 7, 3),
            material: lib.material(texture, white, 1.0, false),
            local: trs(
                Vector3::new(0.0, 0.18, 0.0),
                0.0,
                Vector3::new(1.0, 1.0, 0.85),
            ),
        }],
        "ore" => {
            let mut nugget = lib
                .material("metal", color, 1.0, false)
                .duplicate_resource();
            nugget.set_emission(color.darkened(0.6));
            nugget.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
            vec![
                Part {
                    mesh: sphere(0.5, 0.8, 7, 3),
                    material: lib.material(texture, white, 1.0, false),
                    local: trs(
                        Vector3::new(0.0, 0.22, 0.0),
                        0.0,
                        Vector3::new(1.0, 1.0, 0.9),
                    ),
                },
                Part {
                    mesh: cube(Vector3::new(0.18, 0.18, 0.18)),
                    material: nugget,
                    local: Transform3D::new(
                        Basis::from_euler(EulerOrder::YXZ, Vector3::new(0.6, 0.8, 0.3)),
                        Vector3::new(0.18, 0.5, 0.1),
                    ),
                },
            ]
        }
        "crystal" => {
            let mut mat = lib
                .material(texture, white, 1.0, false)
                .duplicate_resource();
            mat.set_emission(color.darkened(0.3));
            mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
            let shard = cylinder(0.0, 0.16, 1.0, 5);
            let tilt = |x: f32, z: f32, pos: Vector3, s: f32| Part {
                mesh: shard.clone(),
                material: mat.clone(),
                local: Transform3D::new(
                    Basis::from_euler(EulerOrder::YXZ, Vector3::new(x, 0.0, z))
                        * Basis::from_scale(Vector3::splat(s)),
                    pos,
                ),
            };
            vec![
                tilt(0.0, 0.0, Vector3::new(0.0, 0.5, 0.0), 1.1),
                tilt(0.5, 0.3, Vector3::new(0.18, 0.35, 0.12), 0.75),
                tilt(-0.4, -0.5, Vector3::new(-0.2, 0.32, -0.05), 0.7),
                Part {
                    mesh: sphere(0.35, 0.3, 6, 2),
                    material: lib.material("rocky", white, 1.0, false),
                    local: at(Vector3::new(0.0, 0.05, 0.0)),
                },
            ]
        }
        _ => vec![Part {
            mesh: cube(Vector3::new(0.6, 0.6, 0.6)),
            material: lib.material(texture, color, 1.0, false),
            local: at(Vector3::new(0.0, 0.3, 0.0)),
        }],
    }
}

/// A character model. `body` is the mesh whose material reflects armor,
/// `weapon` hangs off a pivot that swings when attacking.
pub struct ActorModel {
    pub root: Gd<Node3D>,
    pub body: Option<Gd<MeshInstance3D>>,
    pub weapon_pivot: Option<Gd<Node3D>>,
    pub weapon: Option<Gd<MeshInstance3D>>,
    pub height: f32,
    pub legs: Vec<Gd<Node3D>>,
}

pub fn humanoid(
    lib: &ContentLibrary,
    texture: &str,
    color: Color,
    skin: Color,
    with_weapon: bool,
) -> ActorModel {
    let mut root = Node3D::new_alloc();
    let skeletal = texture == "bone";
    let cloth = lib.material(texture, color, 1.0, false);
    let leather = lib.material("leather", Color::from_rgb(0.65, 0.65, 0.65), 1.0, false);
    let trim = lib.material("metal", Color::from_rgb(0.72, 0.51, 0.23), 1.0, false);
    let head_mat = lib.material(if with_weapon { "skin" } else { texture }, skin, 1.0, false);
    let body = instance(
        &cylinder(if skeletal { 0.13 } else { 0.31 }, 0.21, 0.55, 10),
        &cloth,
        trs(
            Vector3::new(0.0, 1.02, 0.0),
            0.0,
            Vector3::new(1.0, 1.0, 0.7),
        ),
    );
    root.add_child(&body);
    root.add_child(&instance(
        &sphere(0.18, 0.36, 8, 4),
        &head_mat,
        trs(
            Vector3::new(0.0, 1.53, 0.0),
            0.0,
            Vector3::new(0.88, 1.0, 0.9),
        ),
    ));
    root.add_child(&instance(
        &cylinder(0.075, 0.09, 0.14, 6),
        &head_mat,
        at(Vector3::new(0.0, 1.33, 0.0)),
    ));
    let eye = lib.material("cloth", Color::from_rgb(0.025, 0.025, 0.035), 1.0, false);
    for x in [-0.06, 0.06] {
        root.add_child(&instance(
            &sphere(0.022, 0.032, 6, 3),
            &eye,
            at(Vector3::new(x, 1.55, 0.15)),
        ));
        root.add_child(&instance(
            &cube(Vector3::new(0.065, 0.02, 0.035)),
            &leather,
            at(Vector3::new(x, 1.59, 0.15)),
        ));
    }
    root.add_child(&instance(
        &sphere(0.03, 0.07, 6, 3),
        &head_mat,
        at(Vector3::new(0.0, 1.51, 0.17)),
    ));
    if with_weapon {
        root.add_child(&instance(
            &sphere(0.19, 0.20, 6, 3),
            &leather,
            at(Vector3::new(0.0, 1.68, -0.025)),
        ));
        root.add_child(&instance(
            &cylinder(0.22, 0.22, 0.08, 10),
            &leather,
            trs(
                Vector3::new(0.0, 0.78, 0.0),
                0.0,
                Vector3::new(1.0, 1.0, 0.8),
            ),
        ));
        root.add_child(&instance(
            &cube(Vector3::new(0.12, 0.10, 0.04)),
            &trim,
            at(Vector3::new(0.0, 0.78, 0.19)),
        ));
    }
    if skeletal {
        for y in [0.90, 1.02, 1.14] {
            root.add_child(&instance(
                &sphere(0.24, 0.07, 10, 4),
                &cloth,
                trs(
                    Vector3::new(0.0, y, 0.02),
                    0.0,
                    Vector3::new(1.0, 1.0, 0.65),
                ),
            ));
        }
    }
    let mut legs = Vec::new();
    for side in [-1.0, 1.0] {
        let mut leg = Node3D::new_alloc();
        leg.set_position(Vector3::new(side * 0.13, 0.72, 0.0));
        leg.add_child(&instance(
            &capsule(if skeletal { 0.055 } else { 0.10 }, 0.55),
            &cloth,
            at(Vector3::new(0.0, -0.26, 0.0)),
        ));
        leg.add_child(&instance(
            &capsule(0.105, 0.28),
            if skeletal { &cloth } else { &leather },
            at(Vector3::new(0.0, -0.53, 0.025)),
        ));
        leg.add_child(&instance(
            &sphere(0.12, 0.16, 10, 4),
            if skeletal { &cloth } else { &leather },
            trs(
                Vector3::new(0.0, -0.64, 0.075),
                0.0,
                Vector3::new(0.85, 1.0, 1.5),
            ),
        ));
        root.add_child(&leg);
        legs.push(leg);
        root.add_child(&instance(
            &capsule(if skeletal { 0.045 } else { 0.095 }, 0.50),
            &cloth,
            at(Vector3::new(side * 0.34, 0.98, 0.0)),
        ));
        root.add_child(&instance(
            &sphere(0.085, 0.18, 10, 5),
            &head_mat,
            at(Vector3::new(side * 0.34, 0.70, 0.03)),
        ));
        if with_weapon {
            root.add_child(&instance(
                &sphere(0.18, 0.23, 6, 3),
                &trim,
                at(Vector3::new(side * 0.32, 1.24, 0.0)),
            ));
        }
    }
    if with_weapon {
        let mut cape = SurfaceTool::new_gd();
        cape.begin(godot::classes::mesh::PrimitiveType::TRIANGLES);
        let points = [
            Vector3::new(-0.24, 1.27, -0.18),
            Vector3::new(0.24, 1.27, -0.18),
            Vector3::new(-0.35, 0.28, -0.34),
            Vector3::new(0.35, 0.28, -0.34),
            Vector3::new(0.0, 0.70, -0.43),
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
            cape.add_vertex(points[index]);
        }
        cape.generate_normals();
        if let Some(mesh) = cape.commit() {
            let mut mat = lib
                .material("cloak", Color::WHITE, 1.0, false)
                .duplicate_resource();
            mat.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
            root.add_child(&instance(&mesh.upcast(), &mat, at(Vector3::ZERO)));
        }
    }
    let (mut pivot, mut weapon) = (None, None);
    if with_weapon {
        let mut p = Node3D::new_alloc();
        p.set_position(Vector3::new(0.34, 0.70, 0.05));
        let w = instance(
            &cylinder(0.0, 0.065, 0.8, 4),
            &lib.material("metal", Color::WHITE, 1.0, false),
            Transform3D::new(
                Basis::from_axis_angle(Vector3::RIGHT, std::f32::consts::FRAC_PI_2),
                Vector3::new(0.0, 0.0, 0.4),
            ),
        );
        p.add_child(&w);
        root.add_child(&p);
        pivot = Some(p);
        weapon = Some(w);
    }
    ActorModel {
        root,
        body: Some(body),
        weapon_pivot: pivot,
        weapon,
        height: 1.9,
        legs,
    }
}

pub fn slime(lib: &ContentLibrary, texture: &str, color: Color) -> ActorModel {
    let mut root = Node3D::new_alloc();
    let body = instance(
        &sphere(0.45, 0.65, 14, 7),
        &lib.material(texture, color, 1.0, false),
        at(Vector3::new(0.0, 0.3, 0.0)),
    );
    root.add_child(&body);
    let eye = lib.material("cloth", Color::from_rgb(0.05, 0.1, 0.05), 1.0, false);
    for x in [-0.14, 0.14] {
        root.add_child(&instance(
            &sphere(0.06, 0.12, 6, 3),
            &eye,
            at(Vector3::new(x, 0.42, 0.36)),
        ));
    }
    ActorModel {
        root,
        body: Some(body),
        weapon_pivot: None,
        weapon: None,
        height: 0.8,
        legs: Vec::new(),
    }
}

pub fn wolf(lib: &ContentLibrary, texture: &str, color: Color) -> ActorModel {
    let mut root = Node3D::new_alloc();
    let fur = lib.material(texture, color, 1.0, false);
    let body = instance(
        &sphere(0.30, 0.48, 12, 6),
        &fur,
        trs(
            Vector3::new(0.0, 0.55, 0.0),
            0.0,
            Vector3::new(0.8, 1.0, 1.9),
        ),
    );
    root.add_child(&body);
    root.add_child(&instance(
        &sphere(0.23, 0.38, 10, 5),
        &fur,
        at(Vector3::new(0.0, 0.78, 0.6)),
    ));
    root.add_child(&instance(
        &sphere(0.12, 0.18, 10, 5),
        &fur,
        trs(
            Vector3::new(0.0, 0.72, 0.86),
            0.0,
            Vector3::new(0.8, 1.0, 1.8),
        ),
    ));
    for x in [-0.12, 0.12] {
        root.add_child(&instance(
            &cylinder(0.0, 0.09, 0.25, 4),
            &fur,
            at(Vector3::new(x, 1.0, 0.55)),
        ));
    }
    let eye = lib.material("cloth", Color::from_rgb(0.9, 0.75, 0.2), 1.0, false);
    for x in [-0.09, 0.09] {
        root.add_child(&instance(
            &sphere(0.035, 0.07, 6, 3),
            &eye,
            at(Vector3::new(x, 0.84, 0.8)),
        ));
    }
    let mut legs = Vec::new();
    for (x, z) in [(-0.14, 0.32), (0.14, 0.32), (-0.14, -0.32), (0.14, -0.32)] {
        let mut leg = Node3D::new_alloc();
        leg.set_position(Vector3::new(x, 0.38, z));
        leg.add_child(&instance(
            &capsule(0.065, 0.38),
            &fur,
            at(Vector3::new(0.0, -0.19, 0.0)),
        ));
        root.add_child(&leg);
        legs.push(leg);
    }
    root.add_child(&instance(
        &sphere(0.10, 0.55, 10, 5),
        &fur,
        Transform3D::new(
            Basis::from_axis_angle(Vector3::RIGHT, 1.0),
            Vector3::new(0.0, 0.7, -0.6),
        ),
    ));
    ActorModel {
        root,
        body: Some(body),
        weapon_pivot: None,
        weapon: None,
        height: 1.1,
        legs,
    }
}

/// Builds an enemy or player model from its content model name.
pub fn actor(model: &str, lib: &ContentLibrary, texture: &str, color: Color) -> ActorModel {
    match model {
        "slime" => slime(lib, texture, color),
        "wolf" => wolf(lib, texture, color),
        "player" => humanoid(lib, "cloth", color, Color::from_rgb(1.0, 0.85, 0.72), true),
        _ => humanoid(lib, texture, color, color, false),
    }
}

/// A small floating model for an item lying on the ground.
pub fn ground_item(lib: &ContentLibrary, kind: &str, color: Color) -> Gd<MeshInstance3D> {
    let (mesh, tf) = match kind {
        "gold" => (
            cylinder(0.14, 0.14, 0.05, 10),
            Transform3D::new(Basis::from_axis_angle(Vector3::RIGHT, 1.2), Vector3::ZERO),
        ),
        "potion" => (sphere(0.15, 0.3, 8, 4), at(Vector3::ZERO)),
        "weapon" => (
            cube(Vector3::new(0.06, 0.06, 0.6)),
            Transform3D::new(Basis::from_axis_angle(Vector3::RIGHT, 0.5), Vector3::ZERO),
        ),
        "tool" => (cube(Vector3::new(0.3, 0.08, 0.08)), at(Vector3::ZERO)),
        "armor" => (cube(Vector3::new(0.32, 0.36, 0.12)), at(Vector3::ZERO)),
        _ => (cube(Vector3::new(0.22, 0.22, 0.22)), at(Vector3::ZERO)),
    };
    let tex = match kind {
        "gold" | "weapon" | "tool" | "armor" => "metal",
        _ => "cloth",
    };
    let mut mat = lib.material(tex, color, 1.0, false).duplicate_resource();
    mat.set_emission(color.darkened(0.7));
    mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
    instance(&mesh, &mat, tf)
}
