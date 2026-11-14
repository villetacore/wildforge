//! Dungeons (rebuilt whenever the server reshuffles one) and the portal
//! arches that lead into them from the surface.

use godot::classes::base_material_3d::BillboardMode;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::label_3d::DrawFlags;
use godot::classes::light_3d::Param;
use godot::classes::mesh::PrimitiveType;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    Label3D, Material, MeshInstance3D, MultiMesh, MultiMeshInstance3D, Node3D, SurfaceTool,
};
use godot::prelude::*;

use super::models::{self, at, cube, cylinder};
use super::terrain;
use crate::content::ContentLibrary;
use crate::net::protocol::{DungeonInfo, PortalLink};

fn theme(t: &str) -> (&'static str, &'static str, Color, Color) {
    // wall texture, floor texture, wall tint, torch color
    match t {
        "nest" => (
            "rocky",
            "forest",
            Color::from_rgb(0.55, 0.5, 0.45),
            Color::from_rgb(0.6, 1.0, 0.4),
        ),
        "ice" => (
            "ice",
            "snow",
            Color::from_rgb(0.8, 0.9, 1.0),
            Color::from_rgb(0.5, 0.8, 1.0),
        ),
        "forge" => (
            "stone_blocks",
            "ash",
            Color::from_rgb(0.45, 0.38, 0.35),
            Color::from_rgb(1.0, 0.45, 0.15),
        ),
        "sunken" => (
            "moss",
            "swamp",
            Color::from_rgb(0.6, 0.7, 0.55),
            Color::from_rgb(0.4, 0.9, 0.7),
        ),
        "tomb" => (
            "sandstone",
            "sand",
            Color::from_rgb(0.9, 0.8, 0.65),
            Color::from_rgb(1.0, 0.75, 0.35),
        ),
        _ => (
            "stone_blocks",
            "cobble",
            Color::from_rgb(0.62, 0.6, 0.58),
            Color::from_rgb(1.0, 0.62, 0.3),
        ),
    }
}

fn portal_arch(root: &mut Gd<Node3D>, lib: &ContentLibrary, p: Vector3, label: &str, color: Color) {
    let stone = lib.material_world("stone_blocks", Color::from_rgb(0.7, 0.68, 0.64), 0.45);
    for off in [-0.95, 0.95] {
        root.add_child(&models::instance(
            &cube(Vector3::new(0.45, 2.8, 0.55)),
            &stone,
            at(p + Vector3::new(off, 1.4, 0.0)),
        ));
    }
    root.add_child(&models::instance(
        &cube(Vector3::new(2.4, 0.45, 0.6)),
        &stone,
        at(p + Vector3::UP * 2.85),
    ));
    let mut veil = models::instance(
        &cube(Vector3::new(1.45, 2.5, 0.05)),
        &models::ghost_material(Color::from_rgba(color.r, color.g, color.b, 0.45)),
        at(p + Vector3::UP * 1.3),
    );
    veil.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    root.add_child(&veil);
    let mut l = models::light(color, 1.6, 8.0);
    l.set_position(p + Vector3::UP * 2.0);
    root.add_child(&l);
    let mut text = Label3D::new_alloc();
    text.set_text(label);
    text.set_billboard_mode(BillboardMode::ENABLED);
    text.set_draw_flag(DrawFlags::DISABLE_DEPTH_TEST, true);
    text.set_position(p + Vector3::UP * 3.5);
    text.set_font_size(28);
    text.set_outline_size(9);
    text.set_draw_flag(DrawFlags::FIXED_SIZE, true);
    text.set_pixel_size(0.0012);
    text.set_visibility_range_end(40.0);
    text.set_modulate(Color::from_rgb(0.75, 0.95, 1.0));
    root.add_child(&text);
}

/// Builds the surface arches leading into each dungeon.
pub fn build_surface_portals(
    parent: &mut Gd<Node3D>,
    dungeons: &[DungeonInfo],
    lib: &ContentLibrary,
) {
    let mut root = Node3D::new_alloc();
    root.set_name("Portals");
    for d in dungeons {
        let p = Vector3::new(
            d.surface_x,
            terrain::height(d.surface_x, d.surface_y),
            d.surface_y,
        );
        portal_arch(
            &mut root,
            lib,
            p,
            &format!("{}\nE · Enter", d.name),
            Color::from_rgb(0.4, 0.75, 1.0),
        );
    }
    parent.add_child(&root);
}

pub struct DungeonView {
    pub info: DungeonInfo,
    root: Option<Gd<Node3D>>,
}

impl DungeonView {
    pub fn new(info: DungeonInfo) -> DungeonView {
        DungeonView { info, root: None }
    }

    /// (Re)builds the dungeon from the current terrain codes.
    pub fn rebuild(
        &mut self,
        parent: &mut Gd<Node3D>,
        size: usize,
        codes: &[u8],
        portals: &[PortalLink],
        lib: &ContentLibrary,
    ) {
        if let Some(mut r) = self.root.take() {
            r.queue_free();
        }
        let d = &self.info;
        let (wall_tex, floor_tex, tint, torch) = theme(&d.theme);
        let mut root = Node3D::new_alloc();
        root.set_name(&format!("Dungeon{}", d.index));
        let wall_mat = lib.material_world(wall_tex, tint, 0.45);
        let floor_mat = lib.material(floor_tex, tint.lightened(0.1), 1.0, true);
        let code = |x: usize, y: usize| -> u8 {
            if x >= size || y >= size {
                6
            } else {
                codes[y * size + x]
            }
        };
        let floor_y = terrain::height(d.x as f32 + 1.0, d.y as f32 + 1.0);
        let mut st = SurfaceTool::new_gd();
        st.begin(PrimitiveType::TRIANGLES);
        let mut walls = Vec::new();
        let mut caps = Vec::new();
        let mut torches = 0;
        for y in d.y..d.y + d.size {
            for x in d.x..d.x + d.size {
                let c = code(x, y);
                let open = |cc: u8| matches!(cc, 7..=9);
                if open(c) {
                    for (vx, vy) in [
                        (x, y),
                        (x + 1, y),
                        (x + 1, y + 1),
                        (x, y),
                        (x + 1, y + 1),
                        (x, y + 1),
                    ] {
                        st.set_color(Color::from_rgb(0.95, 0.95, 0.95));
                        st.set_normal(Vector3::UP);
                        st.set_uv(Vector2::new(vx as f32 * 0.5, vy as f32 * 0.5));
                        st.add_vertex(Vector3::new(vx as f32, floor_y, vy as f32));
                    }
                    let p = Vector3::new(x as f32 + 0.5, floor_y, y as f32 + 0.5);
                    if c == 9 {
                        let wood = lib.material("wood", Color::WHITE, 1.0, false);
                        let iron =
                            lib.material("steel", Color::from_rgb(0.75, 0.62, 0.35), 1.0, false);
                        root.add_child(&models::instance(
                            &cube(Vector3::new(0.75, 0.5, 0.5)),
                            &wood,
                            at(p + Vector3::UP * 0.25),
                        ));
                        root.add_child(&models::instance(
                            &cylinder(0.25, 0.25, 0.75, 12),
                            &wood,
                            Transform3D::new(
                                Basis::from_axis_angle(
                                    Vector3::FORWARD,
                                    std::f32::consts::FRAC_PI_2,
                                ),
                                p + Vector3::UP * 0.5,
                            ),
                        ));
                        for off in [-0.26, 0.26] {
                            root.add_child(&models::instance(
                                &cube(Vector3::new(0.07, 0.6, 0.55)),
                                &iron,
                                at(p + Vector3::new(off, 0.32, 0.0)),
                            ));
                        }
                        let mut l = models::light(Color::from_rgb(1.0, 0.75, 0.3), 0.9, 4.0);
                        l.set_position(p + Vector3::UP * 1.2);
                        root.add_child(&l);
                    }
                    continue;
                }
                if c != 6 {
                    continue;
                }
                // Only expose masonry adjacent to playable floor.
                let neighbours = [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)];
                let exposed: Vec<(isize, isize)> = neighbours
                    .iter()
                    .copied()
                    .filter(|(dx, dy)| {
                        let xx = x as isize + dx;
                        let yy = y as isize + dy;
                        xx >= 0 && yy >= 0 && open(code(xx as usize, yy as usize))
                    })
                    .collect();
                if exposed.is_empty() {
                    continue;
                }
                let p = Vector3::new(x as f32 + 0.5, floor_y, y as f32 + 0.5);
                walls.push(at(p + Vector3::UP * 1.9));
                if (x + y) % 2 == 0 {
                    caps.push(at(p + Vector3::UP * 3.95));
                }
                // Torches on some exposed faces.
                if (x * 7 + y * 13) % 11 == 0 && torches < 40 {
                    torches += 1;
                    let (dx, dy) = exposed[0];
                    let tp = p + Vector3::new(dx as f32 * 0.55, 1.9, dy as f32 * 0.55);
                    root.add_child(&models::instance(
                        &cylinder(0.04, 0.03, 0.4, 6),
                        &lib.material("wood", Color::WHITE, 1.0, false),
                        at(tp),
                    ));
                    root.add_child(&models::instance(
                        &models::sphere(0.09, 0.18, 8, 4),
                        &models::glow_material(torch, 4.0),
                        at(tp + Vector3::UP * 0.25),
                    ));
                    let mut l = models::light(torch, 2.2, 9.0);
                    l.set_position(tp + Vector3::UP * 0.4);
                    l.set_param(Param::ATTENUATION, 1.3);
                    root.add_child(&l);
                }
            }
        }
        st.generate_tangents();
        if let Some(mesh) = st.commit() {
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_mesh(&mesh);
            mi.set_material_override(&floor_mat.upcast::<Material>());
            root.add_child(&mi);
        }
        for (tfs, mesh) in [
            (&walls, cube(Vector3::new(1.0, 3.8, 1.0))),
            (&caps, cube(Vector3::new(0.6, 0.4, 0.6))),
        ] {
            if tfs.is_empty() {
                continue;
            }
            let mut mm = MultiMesh::new_gd();
            mm.set_transform_format(TransformFormat::TRANSFORM_3D);
            mm.set_mesh(&mesh);
            mm.set_instance_count(tfs.len() as i32);
            for (i, tf) in tfs.iter().enumerate() {
                mm.set_instance_transform(i as i32, *tf);
            }
            let mut mmi = MultiMeshInstance3D::new_alloc();
            mmi.set_multimesh(&mm);
            mmi.set_material_override(&wall_mat.clone().upcast::<Material>());
            root.add_child(&mmi);
        }
        // Exit arch: the portal link whose source lies inside this dungeon.
        for link in portals {
            let inside = link.x >= d.x as f32
                && link.y >= d.y as f32
                && link.x < (d.x + d.size) as f32
                && link.y < (d.y + d.size) as f32;
            if inside {
                portal_arch(
                    &mut root,
                    lib,
                    Vector3::new(link.x, floor_y, link.y),
                    &format!("{}\nE · Return to the surface", d.name),
                    Color::from_rgb(0.35, 0.65, 0.8),
                );
            }
        }
        parent.add_child(&root);
        self.root = Some(root);
    }
}
