//! Short-lived visual effects: floating combat text and attack rings.

use godot::classes::base_material_3d::{BillboardMode, ShadingMode, Transparency};
use godot::classes::label_3d::DrawFlags;
use godot::classes::{Label3D, Material, MeshInstance3D, Node3D, StandardMaterial3D, TorusMesh};
use godot::prelude::*;

const TEXT_LIFE: f32 = 1.2;
const RING_LIFE: f32 = 0.25;

struct Floater {
    label: Gd<Label3D>,
    start: Vector3,
    color: Color,
    t: f32,
}

struct Ring {
    mesh: Gd<MeshInstance3D>,
    material: Gd<StandardMaterial3D>,
    t: f32,
}

pub struct Effects {
    root: Gd<Node3D>,
    floaters: Vec<Floater>,
    rings: Vec<Ring>,
    torus: Gd<TorusMesh>,
}

impl Effects {
    pub fn new(parent: &mut Gd<Node3D>) -> Effects {
        let mut root = Node3D::new_alloc();
        root.set_name("Effects");
        parent.add_child(&root);
        let mut torus = TorusMesh::new_gd();
        torus.set_inner_radius(0.9);
        torus.set_outer_radius(1.0);
        torus.set_rings(24);
        torus.set_ring_segments(4);
        Effects {
            root,
            floaters: Vec::new(),
            rings: Vec::new(),
            torus,
        }
    }

    pub fn text(&mut self, pos: Vector3, text: &str, color: Color, big: bool) {
        let mut label = Label3D::new_alloc();
        label.set_text(text);
        label.set_billboard_mode(BillboardMode::ENABLED);
        label.set_draw_flag(DrawFlags::DISABLE_DEPTH_TEST, true);
        label.set_font_size(if big { 72 } else { 46 });
        label.set_outline_size(12);
        label.set_pixel_size(0.006);
        label.set_modulate(color);
        label.set_render_priority(3);
        let start = pos + Vector3::new(0.0, 1.9, 0.0);
        label.set_position(start);
        self.root.add_child(&label);
        self.floaters.push(Floater {
            label,
            start,
            color,
            t: 0.0,
        });
    }

    pub fn ring(&mut self, pos: Vector3, color: Color) {
        let mut material = StandardMaterial3D::new_gd();
        material.set_shading_mode(ShadingMode::UNSHADED);
        material.set_transparency(Transparency::ALPHA);
        material.set_albedo(color);
        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_mesh(&self.torus);
        mesh.set_material_override(&material.clone().upcast::<Material>());
        mesh.set_cast_shadows_setting(
            godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF,
        );
        mesh.set_position(pos + Vector3::new(0.0, 0.1, 0.0));
        mesh.set_scale(Vector3::splat(0.4));
        self.root.add_child(&mesh);
        self.rings.push(Ring {
            mesh,
            material,
            t: 0.0,
        });
    }

    pub fn update(&mut self, delta: f32) {
        self.floaters.retain_mut(|f| {
            f.t += delta;
            if f.t >= TEXT_LIFE {
                f.label.queue_free();
                return false;
            }
            f.label
                .set_position(f.start + Vector3::new(0.0, f.t * 1.1, 0.0));
            let mut c = f.color;
            c.a = (TEXT_LIFE - f.t).clamp(0.0, 1.0);
            f.label.set_modulate(c);
            f.label
                .set_outline_modulate(Color::from_rgba(0.0, 0.0, 0.0, c.a));
            true
        });
        self.rings.retain_mut(|r| {
            r.t += delta;
            if r.t >= RING_LIFE {
                r.mesh.queue_free();
                return false;
            }
            let k = r.t / RING_LIFE;
            r.mesh
                .set_scale(Vector3::new(0.4 + k * 1.4, 1.0, 0.4 + k * 1.4));
            let mut c = r.material.get_albedo();
            c.a = 0.7 * (1.0 - k);
            r.material.set_albedo(c);
            true
        });
    }
}
