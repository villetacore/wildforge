//! Town architecture built from the server's structure list: houses in six
//! regional styles, the landmarks of Velhrad (Old Town Hall with its
//! astronomical clock, the twin-spired church, the cathedral and palace on
//! the castle hill, the King's Bridge), city walls, lamps and props.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use godot::classes::base_material_3d::{BillboardMode, CullMode, Feature};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::light_3d::Param;

use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    Label3D, Material, Mesh, MultiMesh, MultiMeshInstance3D, Node3D, OmniLight3D,
    StandardMaterial3D,
};
use godot::prelude::*;

use super::models::{self, at, cube, cylinder, instance, rot, sphere, trs};
use super::terrain;
use crate::content::{parse_color, ContentLibrary};
use crate::net::protocol::Structure;

const FLOOR_H: f32 = 1.55;

pub struct Town {
    lamps: Vec<Gd<OmniLight3D>>,
    windows: Gd<StandardMaterial3D>,
    lanterns: Gd<StandardMaterial3D>,
    hands: Vec<(Gd<Node3D>, f32)>,
    night: f32,
}

struct Ctx<'a> {
    root: Gd<Node3D>,
    lib: &'a ContentLibrary,
    windows: Vec<Transform3D>,
    walls_tf: Vec<Transform3D>,
    merlons: Vec<Transform3D>,
    lamps: Vec<Gd<OmniLight3D>>,
    hands: Vec<(Gd<Node3D>, f32)>,
    lantern_mat: Gd<StandardMaterial3D>,
}

impl Ctx<'_> {
    fn add(
        &mut self,
        mesh: Gd<Mesh>,
        mat: &Gd<StandardMaterial3D>,
        tf: Transform3D,
    ) -> Gd<godot::classes::MeshInstance3D> {
        let mi = instance(&mesh, mat, tf);
        self.root.add_child(&mi);
        mi
    }
    fn mat(&self, tex: &str, color: &str, uv: f32) -> Gd<StandardMaterial3D> {
        match tex {
            "plaster" | "timber" | "wood" | "stone_blocks" | "sandstone" | "marble" | "stone"
            | "moss" => {
                let scale = match tex {
                    "timber" => 0.33,
                    "stone_blocks" | "sandstone" => 0.4,
                    _ => 0.5,
                };
                self.lib
                    .material_world(tex, parse_color(color), scale * uv.max(0.5) * 2.0 / 2.0)
            }
            _ => self.lib.material(tex, parse_color(color), uv, false),
        }
    }
    fn lamp(&mut self, pos: Vector3, color: Color, range: f32) {
        let mut l = models::light(color, 0.0, range);
        l.set_position(pos);
        l.set_param(Param::ATTENUATION, 1.4);
        self.root.add_child(&l);
        self.lamps.push(l);
    }
}

fn gable_roof(w: f32, d: f32, h: f32) -> Gd<Mesh> {
    models::gable_like(w, d, h)
}

fn base_height(s: &Structure) -> f32 {
    let mut h = f32::MAX;
    for (x, y) in [
        (s.x, s.y),
        (s.x + s.w, s.y),
        (s.x, s.y + s.h),
        (s.x + s.w, s.y + s.h),
    ] {
        h = h.min(terrain::height(x as f32, y as f32));
    }
    if h == f32::MAX {
        0.0
    } else {
        h
    }
}

fn wall_texture(style: &str) -> &'static str {
    match style {
        "timber" => "timber",
        "nordic" | "stilts" => "wood",
        "basalt" => "stone_blocks",
        "sandstone" => "sandstone",
        _ => "plaster",
    }
}

fn roof_texture(style: &str) -> &'static str {
    match style {
        "capital" => "roof_tiles",
        "nordic" | "basalt" => "slate",
        "sandstone" => "sandstone",
        _ => "roof",
    }
}

/// Window positions along the facades of a box building.
fn add_windows(ctx: &mut Ctx, center: Vector3, w: f32, d: f32, floors: i32, door: i32) {
    for f in 0..floors {
        let y = center.y + 1.0 + f as f32 * FLOOR_H;
        for (side, len, axis_x) in [(0, w, true), (2, w, true), (1, d, false), (3, d, false)] {
            let n = ((len - 0.4) / 1.25).floor().max(1.0) as i32;
            for i in 0..n {
                let off = (i as f32 + 0.5) / n as f32 * len - len / 2.0;
                if f == 0 && side == door && i == n / 2 {
                    continue; // the door goes here
                }
                let (pos, yaw) = match side {
                    0 => (
                        Vector3::new(center.x + off, y, center.z - d / 2.0 - 0.02),
                        PI,
                    ),
                    2 => (
                        Vector3::new(center.x + off, y, center.z + d / 2.0 + 0.02),
                        0.0,
                    ),
                    1 => (
                        Vector3::new(center.x + w / 2.0 + 0.02, y, center.z + off),
                        FRAC_PI_2,
                    ),
                    _ => (
                        Vector3::new(center.x - w / 2.0 - 0.02, y, center.z + off),
                        -FRAC_PI_2,
                    ),
                };
                let _ = axis_x;
                ctx.windows.push(trs(pos, yaw, Vector3::ONE));
            }
        }
    }
}

fn door_pos(center: Vector3, w: f32, d: f32, door: i32) -> (Vector3, f32) {
    match door {
        0 => (Vector3::new(center.x, center.y, center.z - d / 2.0), PI),
        1 => (
            Vector3::new(center.x + w / 2.0, center.y, center.z),
            FRAC_PI_2,
        ),
        2 => (Vector3::new(center.x, center.y, center.z + d / 2.0), 0.0),
        _ => (
            Vector3::new(center.x - w / 2.0, center.y, center.z),
            -FRAC_PI_2,
        ),
    }
}

/// A box building with walls, plinth, door, windows and a roof by style.
fn house(
    ctx: &mut Ctx,
    s: &Structure,
    floors: i32,
    wall_override: Option<&str>,
) -> (Vector3, f32, f32, f32) {
    let base = base_height(s);
    let (w, d) = (s.w as f32 - 0.12, s.h as f32 - 0.12);
    let hgt = floors as f32 * FLOOR_H + 0.3;
    let center = Vector3::new(
        s.x as f32 + s.w as f32 / 2.0,
        base,
        s.y as f32 + s.h as f32 / 2.0,
    );
    let wall = ctx.mat(
        wall_override.unwrap_or(wall_texture(&s.style)),
        &s.wall,
        0.5,
    );
    let plinth = ctx.mat("stone_blocks", "#8a8478", 0.5);
    let stilts = s.style == "stilts";
    let lift = if stilts { 0.9 } else { 0.0 };
    if stilts {
        let post = ctx.mat("wood", "#6a5a40", 1.0);
        for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            ctx.add(
                cube(Vector3::new(0.18, 1.8, 0.18)),
                &post,
                at(center + Vector3::new(x * (w / 2.0 - 0.15), 0.0, z * (d / 2.0 - 0.15))),
            );
        }
    } else {
        ctx.add(
            cube(Vector3::new(w + 0.1, 1.2, d + 0.1)),
            &plinth,
            at(center + Vector3::new(0.0, -0.4, 0.0)),
        );
    }
    ctx.add(
        cube(Vector3::new(w, hgt, d)),
        &wall,
        at(center + Vector3::new(0.0, lift + hgt / 2.0 + 0.2, 0.0)),
    );
    // Cornice band between floors in the capital.
    if s.style == "capital" && floors > 1 {
        let trim = ctx.mat("plaster", "#f4efe4", 0.5);
        for f in 1..floors {
            ctx.add(
                cube(Vector3::new(w + 0.08, 0.08, d + 0.08)),
                &trim,
                at(center + Vector3::new(0.0, 0.2 + f as f32 * FLOOR_H, 0.0)),
            );
        }
    }
    let top = center.y + lift + hgt + 0.2;
    let roof_mat = {
        let mut m = ctx
            .mat(roof_texture(&s.style), &s.roof, 1.0)
            .duplicate_resource();
        m.set_cull_mode(CullMode::DISABLED);
        m
    };
    if s.style == "sandstone" {
        ctx.add(
            cube(Vector3::new(w + 0.1, 0.15, d + 0.1)),
            &roof_mat,
            at(Vector3::new(center.x, top, center.z)),
        );
        for (x, z, sw, sd) in [
            (0.0, -d / 2.0, w, 0.15),
            (0.0, d / 2.0, w, 0.15),
            (-w / 2.0, 0.0, 0.15, d),
            (w / 2.0, 0.0, 0.15, d),
        ] {
            ctx.add(
                cube(Vector3::new(sw, 0.35, sd)),
                &roof_mat,
                at(Vector3::new(center.x + x, top + 0.25, center.z + z)),
            );
        }
    } else {
        let along_x = w >= d;
        let (rw, rd) = if along_x {
            (w + 0.5, d + 0.7)
        } else {
            (d + 0.5, w + 0.7)
        };
        let rh = (rd * 0.55).clamp(0.9, 3.2) * if s.style == "nordic" { 1.3 } else { 1.0 };
        let yaw = if along_x { 0.0 } else { FRAC_PI_2 };
        ctx.add(
            gable_roof(rw, rd, rh),
            &roof_mat,
            trs(
                Vector3::new(center.x, top - 0.02, center.z),
                yaw,
                Vector3::ONE,
            ),
        );
        // Chimney on some roofs.
        if (s.x * 7 + s.y * 3) % 3 == 0 {
            let brick = ctx.mat("stone_blocks", "#8a5a48", 1.0);
            ctx.add(
                cube(Vector3::new(0.35, 1.2, 0.35)),
                &brick,
                at(Vector3::new(
                    center.x + w * 0.25,
                    top + rh * 0.6,
                    center.z + d * 0.15,
                )),
            );
        }
    }
    // Door and windows.
    let (dp, dyaw) = door_pos(center, w, d, s.door);
    let door = ctx.mat("wood", "#5a3a22", 1.0);
    ctx.add(
        cube(Vector3::new(0.8, 1.55, 0.08)),
        &door,
        trs(dp + Vector3::new(0.0, lift + 0.95, 0.0), dyaw, Vector3::ONE),
    );
    let frame = ctx.mat("stone_blocks", "#c8c0b0", 1.0);
    ctx.add(
        cube(Vector3::new(1.0, 0.12, 0.12)),
        &frame,
        trs(dp + Vector3::new(0.0, lift + 1.78, 0.0), dyaw, Vector3::ONE),
    );
    add_windows(
        ctx,
        center + Vector3::new(0.0, lift, 0.0),
        w,
        d,
        floors,
        s.door,
    );
    (center, w, d, top)
}

fn spire(ctx: &mut Ctx, pos: Vector3, r: f32, h: f32, mat: &Gd<StandardMaterial3D>) {
    ctx.add(
        cylinder(0.0, r, h, 8),
        mat,
        at(pos + Vector3::new(0.0, h / 2.0, 0.0)),
    );
    let gold = models::glow_material(Color::from_rgb(1.0, 0.8, 0.3), 0.3);
    ctx.add(
        sphere(r * 0.18 + 0.05, r * 0.36 + 0.1, 10, 5),
        &gold,
        at(pos + Vector3::new(0.0, h + 0.05, 0.0)),
    );
}

/// A square tower with a pointed roof and four corner spirelets.
fn gothic_tower(
    ctx: &mut Ctx,
    base: Vector3,
    size: f32,
    height: f32,
    wall: &Gd<StandardMaterial3D>,
    roof: &Gd<StandardMaterial3D>,
    spire_h: f32,
) -> f32 {
    ctx.add(
        cube(Vector3::new(size, height, size)),
        wall,
        at(base + Vector3::new(0.0, height / 2.0, 0.0)),
    );
    let top = base.y + height;
    ctx.add(
        cube(Vector3::new(size + 0.2, 0.25, size + 0.2)),
        wall,
        at(Vector3::new(base.x, top, base.z)),
    );
    let mut sp = models::instance(
        &cylinder(0.0, size * 0.62, spire_h, 4),
        roof,
        Transform3D::new(
            Basis::from_axis_angle(Vector3::UP, FRAC_PI_4),
            Vector3::new(base.x, top + spire_h / 2.0, base.z),
        ),
    );
    sp.set_cast_shadows_setting(ShadowCastingSetting::ON);
    ctx.root.add_child(&sp);
    for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        spire(
            ctx,
            Vector3::new(base.x + x * size * 0.45, top, base.z + z * size * 0.45),
            0.22,
            spire_h * 0.35,
            roof,
        );
    }
    let gold = models::glow_material(Color::from_rgb(1.0, 0.8, 0.3), 0.4);
    ctx.add(
        sphere(0.14, 0.28, 10, 5),
        &gold,
        at(Vector3::new(base.x, top + spire_h + 0.1, base.z)),
    );
    top
}

fn clock_face(ctx: &mut Ctx, pos: Vector3, yaw: f32, r: f32, astronomical: bool) {
    let face = models::glow_material(
        if astronomical {
            Color::from_rgb(0.25, 0.4, 0.75)
        } else {
            Color::from_rgb(0.95, 0.92, 0.8)
        },
        0.5,
    );
    let gold = models::glow_material(Color::from_rgb(1.0, 0.78, 0.3), 0.8);
    let basis = Basis::from_axis_angle(Vector3::UP, yaw)
        * Basis::from_axis_angle(Vector3::RIGHT, FRAC_PI_2);
    ctx.add(
        cylinder(r, r, 0.06, 28),
        &face,
        Transform3D::new(basis, pos),
    );
    ctx.add(
        models::torus(r * 0.92, r),
        &gold,
        Transform3D::new(
            basis,
            pos + Basis::from_axis_angle(Vector3::UP, yaw) * Vector3::new(0.0, 0.0, 0.03),
        ),
    );
    if astronomical {
        ctx.add(
            models::torus(r * 0.5, r * 0.56),
            &gold,
            Transform3D::new(
                basis,
                pos + Basis::from_axis_angle(Vector3::UP, yaw) * Vector3::new(0.0, 0.0, 0.04),
            ),
        );
        let sun = models::glow_material(Color::from_rgb(1.0, 0.85, 0.3), 2.0);
        ctx.add(
            sphere(0.1, 0.2, 10, 5),
            &sun,
            at(pos + Basis::from_axis_angle(Vector3::UP, yaw) * Vector3::new(0.0, r * 0.3, 0.06)),
        );
    }
    let dark = ctx.mat("steel", "#1a1a1a", 1.0);
    for (len, speed) in [(r * 0.55, 1.0), (r * 0.85, 12.0)] {
        let mut hand = Node3D::new_alloc();
        hand.set_position(
            pos + Basis::from_axis_angle(Vector3::UP, yaw) * Vector3::new(0.0, 0.0, 0.07),
        );
        hand.set_rotation(Vector3::new(0.0, yaw, 0.0));
        let mut arm = Node3D::new_alloc();
        arm.add_child(&instance(
            &cube(Vector3::new(0.05, len, 0.02)),
            &dark,
            at(Vector3::new(0.0, len / 2.0, 0.0)),
        ));
        hand.add_child(&arm);
        ctx.root.add_child(&hand);
        ctx.hands.push((arm, speed));
    }
}

fn statue(ctx: &mut Ctx, pos: Vector3, yaw: f32, scale: f32) {
    let stone = ctx.mat("stone", "#8a8a80", 1.0);
    let bronze = ctx.mat("steel", "#4a6a5a", 1.0);
    ctx.add(
        cube(Vector3::new(0.6, 0.9, 0.6) * scale),
        &stone,
        at(pos + Vector3::new(0.0, 0.45 * scale, 0.0)),
    );
    let b = pos + Vector3::new(0.0, 0.9 * scale, 0.0);
    ctx.add(
        cylinder(0.2 * scale, 0.32 * scale, 1.0 * scale, 12),
        &bronze,
        at(b + Vector3::new(0.0, 0.5 * scale, 0.0)),
    );
    ctx.add(
        sphere(0.15 * scale, 0.3 * scale, 12, 6),
        &bronze,
        at(b + Vector3::new(0.0, 1.15 * scale, 0.0)),
    );
    ctx.add(
        cylinder(0.03 * scale, 0.03 * scale, 1.0 * scale, 6),
        &bronze,
        rot(
            b + Vector3::new(0.25 * scale, 0.9 * scale, 0.0),
            Vector3::new(0.0, yaw, 0.4),
            Vector3::ONE,
        ),
    );
    ctx.add(
        models::torus(0.15 * scale, 0.2 * scale),
        &models::glow_material(Color::from_rgb(1.0, 0.85, 0.4), 0.6),
        trs(
            b + Vector3::new(0.0, 1.38 * scale, 0.0),
            0.0,
            Vector3::new(1.0, 0.2, 1.0),
        ),
    );
}

fn street_lamp(ctx: &mut Ctx, pos: Vector3) {
    let iron = ctx.mat("steel", "#2a2a2c", 1.0);
    ctx.add(
        cylinder(0.05, 0.08, 2.6, 8),
        &iron,
        at(pos + Vector3::new(0.0, 1.3, 0.0)),
    );
    ctx.add(
        cube(Vector3::new(0.5, 0.05, 0.05)),
        &iron,
        at(pos + Vector3::new(0.2, 2.55, 0.0)),
    );
    let lantern = ctx.lantern_mat.clone();
    ctx.add(
        cube(Vector3::new(0.22, 0.32, 0.22)),
        &lantern,
        at(pos + Vector3::new(0.42, 2.38, 0.0)),
    );
    ctx.add(
        cylinder(0.0, 0.18, 0.15, 4),
        &iron,
        at(pos + Vector3::new(0.42, 2.6, 0.0)),
    );
    ctx.lamp(
        pos + Vector3::new(0.42, 2.2, 0.0),
        Color::from_rgb(1.0, 0.72, 0.38),
        9.0,
    );
}

fn build_structure(ctx: &mut Ctx, s: &Structure) {
    let base = base_height(s);
    let center = Vector3::new(
        s.x as f32 + s.w as f32 / 2.0,
        base,
        s.y as f32 + s.h as f32 / 2.0,
    );
    match s.kind.as_str() {
        "house" | "shop" | "barracks" | "temple" | "tavern" | "smithy" => {
            let floors = s.floors.max(1);
            let (c, w, d, top) = house(ctx, s, floors, None);
            let (dp, dyaw) = door_pos(c, w, d, s.door);
            let out = Basis::from_axis_angle(Vector3::UP, dyaw) * Vector3::new(0.0, 0.0, 1.0);
            match s.kind.as_str() {
                "tavern" => {
                    let wood = ctx.mat("wood", "#7a5232", 1.0);
                    ctx.add(
                        cube(Vector3::new(0.08, 0.08, 0.9)),
                        &wood,
                        trs(
                            dp + out * 0.45 + Vector3::new(0.9, 2.6, 0.0),
                            dyaw,
                            Vector3::ONE,
                        ),
                    );
                    ctx.add(
                        cube(Vector3::new(0.8, 0.55, 0.06)),
                        &ctx.mat("wood", "#c8a060", 1.0),
                        trs(
                            dp + out * 0.8 + Vector3::new(0.9, 2.2, 0.0),
                            dyaw,
                            Vector3::ONE,
                        ),
                    );
                    sign(
                        ctx,
                        dp + out * 0.86 + Vector3::new(0.9, 2.2, 0.0),
                        dyaw,
                        &s.name,
                    );
                    ctx.lamp(
                        dp + out * 1.0 + Vector3::new(0.0, 2.2, 0.0),
                        Color::from_rgb(1.0, 0.65, 0.3),
                        10.0,
                    );
                    let barrel = ctx.mat("wood", "#8a6a4a", 1.0);
                    for k in 0..3 {
                        ctx.add(
                            cylinder(0.28, 0.28, 0.7, 12),
                            &barrel,
                            at(dp + out * 0.5 + Vector3::new(-1.6 - k as f32 * 0.62, 0.35, 0.0)),
                        );
                    }
                }
                "smithy" => {
                    let brick = ctx.mat("stone_blocks", "#6a4a3a", 1.0);
                    ctx.add(
                        cube(Vector3::new(0.6, 2.2, 0.6)),
                        &brick,
                        at(Vector3::new(c.x - w * 0.3, top + 0.6, c.z)),
                    );
                    let glow = models::glow_material(Color::from_rgb(1.0, 0.45, 0.1), 3.0);
                    ctx.add(
                        cube(Vector3::new(0.9, 0.6, 0.15)),
                        &glow,
                        trs(
                            dp + out * 0.06 + Vector3::new(1.2, 0.8, 0.0),
                            dyaw,
                            Vector3::ONE,
                        ),
                    );
                    let mut forge = models::light(Color::from_rgb(1.0, 0.5, 0.15), 2.5, 7.0);
                    forge.set_position(dp + out * 0.8 + Vector3::new(1.2, 1.0, 0.0));
                    ctx.root.add_child(&forge);
                    let iron = ctx.mat("steel", "#3a3a3e", 1.0);
                    ctx.add(
                        cube(Vector3::new(0.6, 0.25, 0.3)),
                        &iron,
                        at(dp + out * 1.6 + Vector3::new(-1.0, 0.65, 0.0)),
                    );
                    ctx.add(
                        cube(Vector3::new(0.3, 0.5, 0.25)),
                        &iron,
                        at(dp + out * 1.6 + Vector3::new(-1.0, 0.25, 0.0)),
                    );
                }
                "shop" | "barracks" => {
                    let cloth = ctx.mat(
                        "cloth",
                        if s.kind == "shop" {
                            "#b03a2a"
                        } else {
                            "#3a4a8a"
                        },
                        1.0,
                    );
                    ctx.add(
                        cube(Vector3::new(w.min(2.4), 0.06, 0.9)),
                        &cloth,
                        Transform3D::new(
                            Basis::from_axis_angle(Vector3::UP, dyaw)
                                * Basis::from_axis_angle(Vector3::RIGHT, 0.35),
                            dp + out * 0.45 + Vector3::new(0.0, 2.15, 0.0),
                        ),
                    );
                }
                "temple" => {
                    let dome = ctx.mat("slate", "#c8a050", 1.0);
                    ctx.add(
                        sphere(w.min(d) * 0.35, w.min(d) * 0.7, 20, 10),
                        &dome,
                        at(Vector3::new(c.x, top + 0.6, c.z)),
                    );
                }
                _ => {}
            }
        }
        "stable" => {
            let wood = ctx.mat("wood", "#8a6a48", 1.0);
            let roof = ctx.mat(roof_texture(&s.style), &s.roof, 1.0);
            let (w, d) = (s.w as f32, s.h as f32);
            for (x, z) in [
                (-1.0, -1.0),
                (1.0, -1.0),
                (-1.0, 1.0),
                (1.0, 1.0),
                (0.0, -1.0),
                (0.0, 1.0),
            ] {
                ctx.add(
                    cube(Vector3::new(0.18, 2.4, 0.18)),
                    &wood,
                    at(center + Vector3::new(x * (w / 2.0 - 0.2), 1.2, z * (d / 2.0 - 0.2))),
                );
            }
            ctx.add(
                cube(Vector3::new(w, 2.0, 0.12)),
                &wood,
                at(center + Vector3::new(0.0, 1.0, -d / 2.0 + 0.1)),
            );
            ctx.add(
                gable_roof(w + 0.5, d + 0.6, 1.3),
                &roof,
                at(center + Vector3::new(0.0, 2.4, 0.0)),
            );
            let hay = ctx.mat("cloth", "#d8b860", 1.0);
            for k in 0..3 {
                ctx.add(
                    cube(Vector3::new(0.9, 0.5, 0.6)),
                    &hay,
                    at(center + Vector3::new(-w / 3.0 + k as f32 * 0.95, 0.25, 0.0)),
                );
            }
        }
        "townhall" => {
            let (c, w, d, top) = house(ctx, s, s.floors.max(2), None);
            // The Old Town Hall tower rises at the west end, clock on the square side.
            let wall = ctx.mat("stone_blocks", "#d8ccb4", 0.5);
            let roof = ctx.mat("slate", "#3a3a40", 1.0);
            let t = Vector3::new(c.x - w / 2.0 + 1.2, c.y, c.z);
            let tower_h = top - c.y + 7.0;
            let ttop = gothic_tower(ctx, t, 2.4, tower_h, &wall, &roof, 4.0);
            let face_z = c.z + d / 2.0 + 0.05;
            let _ = face_z;
            clock_face(
                ctx,
                Vector3::new(t.x, ttop - 1.6, t.z + 1.25),
                0.0,
                0.8,
                false,
            );
            clock_face(
                ctx,
                Vector3::new(t.x + 1.25, c.y + 3.2, t.z + 1.25),
                0.0,
                0.85,
                true,
            );
            ctx.add(
                cube(Vector3::new(0.3, 0.3, 0.3)),
                &models::glow_material(Color::from_rgb(1.0, 0.8, 0.3), 1.0),
                at(Vector3::new(t.x + 1.25, c.y + 4.3, t.z + 1.25)),
            );
        }
        "church" => {
            let wall = ctx.mat("stone_blocks", &s.wall, 0.5);
            let roof = ctx.mat("slate", "#2a2a32", 1.0);
            let (w, d) = (s.w as f32 - 0.1, s.h as f32 - 0.1);
            let hgt = 8.0;
            ctx.add(
                cube(Vector3::new(w, hgt, d)),
                &wall,
                at(center + Vector3::new(0.0, hgt / 2.0 - 0.3, 0.0)),
            );
            ctx.add(
                gable_roof(d + 0.4, w + 0.6, 3.0),
                &roof,
                trs(
                    center + Vector3::new(0.0, hgt - 0.3, 0.0),
                    FRAC_PI_2,
                    Vector3::ONE,
                ),
            );
            // Two tall black spires with spirelets, facing the square.
            let front = if s.door == 3 { -1.0 } else { 1.0 };
            for side in [-1.0, 1.0] {
                let t =
                    center + Vector3::new(front * (w / 2.0 - 1.0), -0.3, side * (d / 2.0 - 1.0));
                gothic_tower(ctx, t, 1.9, 13.0, &wall, &roof, 9.0);
            }
            let rose = models::glow_material(Color::from_rgb(0.9, 0.5, 0.3), 1.2);
            ctx.add(
                cylinder(0.9, 0.9, 0.1, 24),
                &rose,
                Transform3D::new(
                    Basis::from_axis_angle(Vector3::FORWARD, FRAC_PI_2),
                    center + Vector3::new(front * (w / 2.0 + 0.04), 5.5, 0.0),
                ),
            );
            ctx.windows.push(trs(
                center + Vector3::new(front * (w / 2.0 + 0.03), 1.0, 0.0),
                front * FRAC_PI_2,
                Vector3::new(1.6, 2.6, 1.0),
            ));
        }
        "cathedral" => {
            let wall = ctx.mat("stone_blocks", &s.wall, 0.5);
            let copper = ctx.mat("slate", "#5a9a7a", 1.0);
            let (w, d) = (s.w as f32 - 0.1, s.h as f32 - 0.1);
            let hgt = 10.0;
            ctx.add(
                cube(Vector3::new(w, hgt, d)),
                &wall,
                at(center + Vector3::new(0.0, hgt / 2.0, 0.0)),
            );
            ctx.add(
                gable_roof(w + 0.4, d + 0.6, 3.5),
                &copper,
                at(center + Vector3::new(0.0, hgt, 0.0)),
            );
            // Buttresses.
            for i in 0..5 {
                let x = center.x - w / 2.0 + 1.0 + i as f32 * (w - 2.0) / 4.0;
                for side in [-1.0, 1.0] {
                    ctx.add(
                        cube(Vector3::new(0.4, hgt * 0.8, 0.8)),
                        &wall,
                        at(Vector3::new(
                            x,
                            center.y + hgt * 0.4,
                            center.z + side * (d / 2.0 + 0.35),
                        )),
                    );
                }
            }
            // Main tower with green copper helm, two front towers.
            gothic_tower(
                ctx,
                Vector3::new(center.x + w / 2.0 - 1.4, center.y, center.z),
                2.6,
                17.0,
                &wall,
                &copper,
                7.0,
            );
            for side in [-1.0, 1.0] {
                gothic_tower(
                    ctx,
                    Vector3::new(
                        center.x - w / 2.0 + 0.9,
                        center.y,
                        center.z + side * (d / 2.0 - 0.8),
                    ),
                    1.6,
                    13.0,
                    &wall,
                    &copper,
                    6.0,
                );
            }
            let rose = models::glow_material(Color::from_rgb(0.4, 0.5, 1.0), 1.5);
            ctx.add(
                cylinder(1.1, 1.1, 0.1, 28),
                &rose,
                Transform3D::new(
                    Basis::from_axis_angle(Vector3::FORWARD, FRAC_PI_2),
                    center + Vector3::new(-w / 2.0 - 0.04, 7.0, 0.0),
                ),
            );
            ctx.lamp(
                center + Vector3::new(-w / 2.0 - 1.5, 3.0, 0.0),
                Color::from_rgb(0.7, 0.75, 1.0),
                10.0,
            );
        }
        "palace" => {
            let (c, w, d, top) = house(ctx, s, s.floors.max(3), None);
            let wall = ctx.mat("plaster", &s.wall, 0.5);
            let (dp, dyaw) = door_pos(c, w, d, s.door);
            // Pediment with columns over the entrance.
            let out = Basis::from_axis_angle(Vector3::UP, dyaw) * Vector3::new(0.0, 0.0, 1.0);
            for k in -2..=2 {
                let side = Basis::from_axis_angle(Vector3::UP, dyaw)
                    * Vector3::new(k as f32 * 0.7, 0.0, 0.0);
                ctx.add(
                    cylinder(0.14, 0.16, 3.6, 12),
                    &ctx.mat("marble", "#f4f0e8", 1.0),
                    at(dp + out * 0.6 + side + Vector3::new(0.0, 1.8, 0.0)),
                );
            }
            ctx.add(
                cube(Vector3::new(3.6, 0.4, 1.2)),
                &wall,
                trs(
                    dp + out * 0.6 + Vector3::new(0.0, 3.8, 0.0),
                    dyaw,
                    Vector3::ONE,
                ),
            );
            for k in [-1.0, 1.0] {
                let banner = ctx.mat("cloth", "#8a1a24", 1.0);
                let side =
                    Basis::from_axis_angle(Vector3::UP, dyaw) * Vector3::new(k * 2.4, 0.0, 0.0);
                ctx.add(
                    cube(Vector3::new(0.7, 2.0, 0.04)),
                    &banner,
                    trs(
                        dp + out * 0.08 + side + Vector3::new(0.0, 3.2, 0.0),
                        dyaw,
                        Vector3::ONE,
                    ),
                );
            }
            let flag = ctx.mat("cloth", "#d4a83a", 1.0);
            ctx.add(
                cylinder(0.04, 0.04, 2.5, 6),
                &ctx.mat("steel", "#3a3a3a", 1.0),
                at(Vector3::new(c.x, top + 3.5, c.z)),
            );
            ctx.add(
                cube(Vector3::new(1.0, 0.6, 0.03)),
                &flag,
                at(Vector3::new(c.x + 0.5, top + 4.4, c.z)),
            );
        }
        "magetower" => {
            let stone = ctx.mat("stone_blocks", &s.wall, 0.5);
            let roof = ctx.mat("slate", &s.roof, 1.0);
            let r = s.w.min(s.h) as f32 * 0.45;
            let hgt = s.floors.max(4) as f32 * FLOOR_H + 1.0;
            ctx.add(
                cylinder(r * 0.92, r, hgt, 24),
                &stone,
                at(center + Vector3::new(0.0, hgt / 2.0 - 0.3, 0.0)),
            );
            for f in 1..s.floors.max(4) {
                ctx.add(
                    models::torus(r * 0.95, r * 1.08),
                    &stone,
                    trs(
                        center + Vector3::new(0.0, f as f32 * FLOOR_H, 0.0),
                        0.0,
                        Vector3::new(1.0, 0.6, 1.0),
                    ),
                );
            }
            ctx.add(
                cylinder(0.0, r * 1.25, 4.0, 24),
                &roof,
                at(center + Vector3::new(0.0, hgt + 1.7, 0.0)),
            );
            let orb = models::glow_material(Color::from_rgb(0.5, 0.6, 1.0), 4.0);
            ctx.add(
                sphere(0.35, 0.7, 16, 8),
                &orb,
                at(center + Vector3::new(0.0, hgt + 4.2, 0.0)),
            );
            let mut l = models::light(Color::from_rgb(0.5, 0.6, 1.0), 3.0, 14.0);
            l.set_position(center + Vector3::new(0.0, hgt + 4.0, 0.0));
            ctx.root.add_child(&l);
            for f in 0..s.floors.max(4) {
                for k in 0..6 {
                    let a = k as f32 / 6.0 * TAU + f as f32 * 0.5;
                    ctx.windows.push(trs(
                        center
                            + Vector3::new(
                                a.sin() * (r * 0.93 + 0.02),
                                1.0 + f as f32 * FLOOR_H,
                                a.cos() * (r * 0.93 + 0.02),
                            ),
                        a,
                        Vector3::ONE,
                    ));
                }
            }
            let (dp, dyaw) = door_pos(center, r * 2.0, r * 2.0, s.door);
            ctx.add(
                cube(Vector3::new(0.9, 1.7, 0.1)),
                &ctx.mat("wood", "#3a2a4a", 1.0),
                trs(dp + Vector3::new(0.0, 0.85, 0.0), dyaw, Vector3::ONE),
            );
        }
        "stall" => {
            let wood = ctx.mat("wood", &s.wall, 1.0);
            let cloth = {
                let mut m = ctx.mat("cloth", &s.roof, 1.0).duplicate_resource();
                m.set_cull_mode(CullMode::DISABLED);
                m
            };
            let (w, d) = (s.w as f32, s.h as f32);
            ctx.add(
                cube(Vector3::new(w - 0.2, 0.9, d - 0.2)),
                &wood,
                at(center + Vector3::new(0.0, 0.45, 0.0)),
            );
            for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                ctx.add(
                    cube(Vector3::new(0.08, 2.2, 0.08)),
                    &wood,
                    at(center + Vector3::new(x * (w / 2.0 - 0.1), 1.1, z * (d / 2.0 - 0.1))),
                );
            }
            ctx.add(
                gable_roof(w + 0.3, d + 0.5, 0.5),
                &cloth,
                at(center + Vector3::new(0.0, 2.2, 0.0)),
            );
            let goods = ["#c03030", "#e0b030", "#40a040", "#e08030"];
            for (k, g) in goods.iter().enumerate() {
                ctx.add(
                    sphere(0.12, 0.24, 10, 5),
                    &ctx.mat("cloth", g, 1.0),
                    at(center + Vector3::new(-w / 2.0 + 0.4 + k as f32 * 0.35, 0.98, 0.0)),
                );
            }
        }
        "monument" => statue(ctx, center, 0.0, 1.6),
        "well" => {
            let stone = ctx.mat("stone_blocks", "#8a8a88", 1.0);
            let wood = ctx.mat("wood", "#6a4a30", 1.0);
            ctx.add(
                cylinder(0.55, 0.6, 0.8, 18),
                &stone,
                at(center + Vector3::new(0.0, 0.4, 0.0)),
            );
            ctx.add(
                cylinder(0.45, 0.45, 0.05, 18),
                &ctx.mat("water", "#406080", 1.0),
                at(center + Vector3::new(0.0, 0.7, 0.0)),
            );
            for x in [-0.5, 0.5] {
                ctx.add(
                    cube(Vector3::new(0.1, 1.8, 0.1)),
                    &wood,
                    at(center + Vector3::new(x, 0.9, 0.0)),
                );
            }
            ctx.add(
                gable_roof(1.4, 1.1, 0.45),
                &ctx.mat(roof_texture(&s.style), "#8a4a30", 1.0),
                at(center + Vector3::new(0.0, 1.8, 0.0)),
            );
        }
        "lamp" => street_lamp(ctx, center),
        "table" => {
            let wood = ctx.mat("wood", "#8a6a48", 1.0);
            ctx.add(
                cube(Vector3::new(1.4, 0.08, 0.7)),
                &wood,
                at(center + Vector3::new(0.0, 0.75, 0.0)),
            );
            for (x, z) in [(-0.6, -0.28), (0.6, -0.28), (-0.6, 0.28), (0.6, 0.28)] {
                ctx.add(
                    cube(Vector3::new(0.07, 0.75, 0.07)),
                    &wood,
                    at(center + Vector3::new(x, 0.37, z)),
                );
            }
            for z in [-0.65, 0.65] {
                ctx.add(
                    cube(Vector3::new(1.4, 0.06, 0.28)),
                    &wood,
                    at(center + Vector3::new(0.0, 0.45, z)),
                );
            }
            let mug = ctx.mat("steel", "#a08060", 1.0);
            for x in [-0.4, 0.1, 0.45] {
                ctx.add(
                    cylinder(0.06, 0.06, 0.15, 10),
                    &mug,
                    at(center + Vector3::new(x, 0.86, 0.1)),
                );
            }
        }
        "kings_bridge" => {
            let stone = ctx.mat("stone_blocks", "#b8ac98", 0.5);
            let roof = ctx.mat("slate", "#2a2a32", 1.0);
            let (x0, x1) = (s.x as f32, (s.x + s.w) as f32);
            let (zt, zb) = (s.y as f32, (s.y + s.h) as f32);
            let y = 0.05;
            // Deck and arches.
            ctx.add(
                cube(Vector3::new(x1 - x0 + 1.0, 0.5, zb - zt)),
                &stone,
                at(Vector3::new((x0 + x1) / 2.0, y - 0.25, (zt + zb) / 2.0)),
            );
            let mut x = x0 + 0.5;
            while x < x1 {
                ctx.add(
                    cube(Vector3::new(0.8, 1.6, zb - zt + 0.2)),
                    &stone,
                    at(Vector3::new(x, y - 1.1, (zt + zb) / 2.0)),
                );
                x += 3.0;
            }
            // Parapets with baroque statues every three tiles.
            for z in [zt + 0.1, zb - 0.1] {
                ctx.add(
                    cube(Vector3::new(x1 - x0, 0.6, 0.2)),
                    &stone,
                    at(Vector3::new((x0 + x1) / 2.0, y + 0.3, z)),
                );
                let mut x = x0 + 1.5;
                let mut k = 0;
                while x < x1 - 0.5 {
                    statue(
                        ctx,
                        Vector3::new(x, y + 0.6, z),
                        if z < (zt + zb) / 2.0 { 0.0 } else { PI },
                        0.75,
                    );
                    if k % 2 == 0 {
                        ctx.lamp(
                            Vector3::new(x + 0.75, y + 2.4, z),
                            Color::from_rgb(1.0, 0.75, 0.4),
                            7.0,
                        );
                    }
                    x += 3.0;
                    k += 1;
                }
            }
            // Gate towers at both ends of the bridge.
            for (gx, dir) in [(x0 - 1.4, -1.0), (x1 + 1.4, 1.0)] {
                let gz = (zt + zb) / 2.0;
                let gy = terrain::height(gx, gz);
                for side in [-1.0, 1.0] {
                    ctx.add(
                        cube(Vector3::new(2.2, 5.0, 1.1)),
                        &stone,
                        at(Vector3::new(gx, gy + 2.5, gz + side * 2.4)),
                    );
                }
                ctx.add(
                    cube(Vector3::new(2.4, 4.0, 5.9)),
                    &stone,
                    at(Vector3::new(gx, gy + 7.0, gz)),
                );
                ctx.add(
                    cylinder(0.0, 2.6, 4.5, 4),
                    &roof,
                    Transform3D::new(
                        Basis::from_axis_angle(Vector3::UP, FRAC_PI_4)
                            * Basis::from_scale(Vector3::new(0.75, 1.0, 1.6)),
                        Vector3::new(gx, gy + 11.25, gz),
                    ),
                );
                for side in [-1.0, 1.0] {
                    spire(
                        ctx,
                        Vector3::new(gx, gy + 9.0, gz + side * 2.5),
                        0.3,
                        2.2,
                        &roof,
                    );
                }
                let _ = dir;
            }
            if !s.name.is_empty() {
                sign(
                    ctx,
                    Vector3::new(
                        x0 - 1.4 - 1.25,
                        terrain::height(x0 - 2.0, (zt + zb) / 2.0) + 6.0,
                        (zt + zb) / 2.0,
                    ),
                    -FRAC_PI_2,
                    &s.name,
                );
            }
        }
        _ => {}
    }
}

fn sign(ctx: &mut Ctx, pos: Vector3, yaw: f32, text: &str) {
    if text.is_empty() {
        return;
    }
    let mut l = Label3D::new_alloc();
    l.set_text(text);
    l.set_font_size(28);
    l.set_pixel_size(0.008);
    l.set_outline_size(8);
    l.set_modulate(Color::from_rgb(1.0, 0.92, 0.7));
    l.set_billboard_mode(BillboardMode::DISABLED);
    l.set_position(pos);
    l.set_rotation(Vector3::new(0.0, yaw, 0.0));
    ctx.root.add_child(&l);
}

fn multimesh(
    root: &mut Gd<Node3D>,
    mesh: &Gd<Mesh>,
    mat: &Gd<StandardMaterial3D>,
    tfs: &[Transform3D],
    shadows: bool,
) {
    if tfs.is_empty() {
        return;
    }
    let mut mm = MultiMesh::new_gd();
    mm.set_transform_format(TransformFormat::TRANSFORM_3D);
    mm.set_mesh(mesh);
    mm.set_instance_count(tfs.len() as i32);
    for (i, tf) in tfs.iter().enumerate() {
        mm.set_instance_transform(i as i32, *tf);
    }
    let mut mmi = MultiMeshInstance3D::new_alloc();
    mmi.set_multimesh(&mm);
    mmi.set_material_override(&mat.clone().upcast::<Material>());
    if !shadows {
        mmi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    }
    root.add_child(&mmi);
}

impl Town {
    pub fn build(
        parent: &mut Gd<Node3D>,
        structures: &[Structure],
        size: usize,
        codes: &[u8],
        lib: &ContentLibrary,
    ) -> Town {
        let mut root = Node3D::new_alloc();
        root.set_name("Towns");
        parent.add_child(&root);
        let mut window_mat = lib
            .material("window", Color::WHITE, 1.0, false)
            .duplicate_resource();
        window_mat.set_feature(Feature::EMISSION, true);
        window_mat.set_emission(Color::from_rgb(1.0, 0.7, 0.35));
        window_mat.set_emission_energy_multiplier(0.0);
        window_mat.set_albedo(Color::from_rgb(0.35, 0.32, 0.3));
        let lantern_mat = models::glow_material(Color::from_rgb(1.0, 0.75, 0.4), 0.5);
        let mut ctx = Ctx {
            root,
            lib,
            windows: Vec::new(),
            walls_tf: Vec::new(),
            merlons: Vec::new(),
            lamps: Vec::new(),
            hands: Vec::new(),
            lantern_mat: lantern_mat.clone(),
        };
        for s in structures {
            build_structure(&mut ctx, s);
        }
        // City walls from terrain tiles, with merlons on every other tile.
        for y in 0..size {
            for x in 0..size {
                if codes[y * size + x] != 17 {
                    continue;
                }
                let h = terrain::height(x as f32 + 0.5, y as f32 + 0.5);
                ctx.walls_tf
                    .push(at(Vector3::new(x as f32 + 0.5, h + 1.4, y as f32 + 0.5)));
                if (x + y) % 2 == 0 {
                    ctx.merlons
                        .push(at(Vector3::new(x as f32 + 0.5, h + 3.25, y as f32 + 0.5)));
                }
            }
        }
        let wall_mat = lib.material_world("stone_blocks", Color::from_rgb(0.75, 0.72, 0.66), 0.4);
        let window_mesh = cube(Vector3::new(0.42, 0.62, 0.05));
        let Ctx {
            mut root,
            windows,
            walls_tf,
            merlons,
            lamps,
            hands,
            ..
        } = ctx;
        multimesh(
            &mut root,
            &cube(Vector3::new(1.02, 4.0, 1.02)),
            &wall_mat,
            &walls_tf,
            true,
        );
        multimesh(
            &mut root,
            &cube(Vector3::new(0.7, 0.7, 0.7)),
            &wall_mat,
            &merlons,
            true,
        );
        multimesh(&mut root, &window_mesh, &window_mat, &windows, false);
        // Shutters frame the windows.
        let shutter = lib.material("wood", Color::from_rgb(0.35, 0.45, 0.35), 1.0, false);
        let shutters: Vec<Transform3D> = windows
            .iter()
            .flat_map(|tf| [-0.3f32, 0.3].map(|x| *tf * at(Vector3::new(x, 0.0, -0.01))))
            .collect();
        multimesh(
            &mut root,
            &cube(Vector3::new(0.16, 0.64, 0.04)),
            &shutter,
            &shutters,
            false,
        );
        Town {
            lamps,
            windows: window_mat,
            lanterns: lantern_mat,
            hands,
            night: -1.0,
        }
    }

    /// Lights windows and lamps at night, turns the clock hands.
    pub fn update(&mut self, tod: f32, night: f32) {
        for (hand, speed) in &mut self.hands {
            hand.set_rotation(Vector3::new(0.0, 0.0, -tod * TAU * 2.0 * *speed));
        }
        if (night - self.night).abs() < 0.02 {
            return;
        }
        self.night = night;
        self.windows.set_emission_energy_multiplier(night * 0.75);
        self.lanterns
            .set_emission_energy_multiplier(0.3 + night * 2.0);
        for l in &mut self.lamps {
            l.set_param(Param::ENERGY, night * 0.9);
            l.set_visible(night > 0.05);
        }
    }
}
