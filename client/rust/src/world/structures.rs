//! Dungeon masonry, portal arches, caches and local lights.
use super::{models, terrain};
use crate::content::ContentLibrary;
use godot::classes::base_material_3d::BillboardMode;
use godot::classes::light_3d::Param;
use godot::classes::{Label3D, Node3D, OmniLight3D};
use godot::prelude::*;

pub fn build(root: &mut Gd<Node3D>, size: usize, codes: &[u8], lib: &ContentLibrary) {
    let stone = lib.material(
        "stone_blocks",
        Color::from_rgb(0.65, 0.64, 0.62),
        1.0,
        false,
    );
    let wood = lib.material("wood", Color::WHITE, 1.0, false);
    let wall = models::cube(Vector3::new(1.0, 3.5, 1.0));
    for y in 0..size {
        for x in 0..size {
            let code = codes[y * size + x];
            let p = Vector3::new(
                x as f32 + 0.5,
                terrain::height(x as f32 + 0.5, y as f32 + 0.5),
                y as f32 + 0.5,
            );
            if code == 6 {
                // Only expose masonry adjacent to playable floor.
                let exposed =
                    [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|(dx, dy)| {
                            let xx = x as isize + dx;
                            let yy = y as isize + dy;
                            xx >= 0
                                && yy >= 0
                                && xx < size as isize
                                && yy < size as isize
                                && (7..=9).contains(&codes[yy as usize * size + xx as usize])
                        });
                if exposed {
                    root.add_child(&models::instance(
                        &wall,
                        &stone,
                        models::at(p + Vector3::UP * 1.75),
                    ));
                }
            }
            if code == 8 {
                for off in [-0.85, 0.85] {
                    root.add_child(&models::instance(
                        &models::cube(Vector3::new(0.4, 2.6, 0.5)),
                        &stone,
                        models::at(p + Vector3::new(off, 1.3, 0.0)),
                    ));
                }
                root.add_child(&models::instance(
                    &models::cube(Vector3::new(2.1, 0.4, 0.5)),
                    &stone,
                    models::at(p + Vector3::UP * 2.6),
                ));
                let mut label = Label3D::new_alloc();
                label.set_text("E · Enter / Return");
                label.set_billboard_mode(BillboardMode::ENABLED);
                label.set_position(p + Vector3::UP * 3.1);
                label.set_font_size(32);
                label.set_pixel_size(0.006);
                root.add_child(&label);
                light(
                    root,
                    p + Vector3::UP * 2.0,
                    Color::from_rgb(0.35, 0.65, 0.8),
                    7.0,
                );
            }
            if code == 9 {
                root.add_child(&models::instance(
                    &models::cube(Vector3::new(0.7, 0.5, 0.5)),
                    &wood,
                    models::at(p + Vector3::UP * 0.25),
                ));
                let iron = lib.material("metal", Color::from_rgb(0.7, 0.65, 0.5), 1.0, false);
                for off in [-0.24, 0.24] {
                    root.add_child(&models::instance(
                        &models::cube(Vector3::new(0.07, 0.52, 0.52)),
                        &iron,
                        models::at(p + Vector3::new(off, 0.26, 0.0)),
                    ));
                }
                light(
                    root,
                    p + Vector3::UP * 1.8,
                    Color::from_rgb(1.0, 0.65, 0.3),
                    8.0,
                );
            }
        }
    }
}

fn light(root: &mut Gd<Node3D>, pos: Vector3, color: Color, range: f32) {
    let mut light = OmniLight3D::new_alloc();
    light.set_position(pos);
    light.set_color(color);
    light.set_param(Param::ENERGY, 1.2);
    light.set_param(Param::RANGE, range);
    root.add_child(&light);
}
