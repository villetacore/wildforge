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
    size: f32,
    life: f32,
}

struct Slash {
    mesh: Gd<MeshInstance3D>,
    material: Gd<StandardMaterial3D>,
    delay: f32,
    t: f32,
}

struct Bolt {
    node: Gd<Node3D>,
    from: Vector3,
    to: Vector3,
    t: f32,
}

pub struct Effects {
    root: Gd<Node3D>,
    floaters: Vec<Floater>,
    rings: Vec<Ring>,
    bolts: Vec<Bolt>,
    slashes: Vec<Slash>,
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
            bolts: Vec::new(),
            slashes: Vec::new(),
            torus,
        }
    }

    pub fn text(&mut self, pos: Vector3, text: &str, color: Color, big: bool) {
        let mut label = Label3D::new_alloc();
        label.set_text(text);
        label.set_billboard_mode(BillboardMode::ENABLED);
        label.set_draw_flag(DrawFlags::DISABLE_DEPTH_TEST, true);
        label.set_font_size(if big { 52 } else { 34 });
        label.set_outline_size(10);
        label.set_draw_flag(DrawFlags::FIXED_SIZE, true);
        label.set_pixel_size(0.0013);
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
        self.ring_ex(pos, color, 1.4, RING_LIFE);
    }

    /// A spell projectile: a glowing orb with a light flying from -> to.
    pub fn bolt(&mut self, from: Vector3, to: Vector3, color: Color) {
        let mut node = Node3D::new_alloc();
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_albedo(color);
        mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        mat.set_emission(color);
        mat.set_emission_energy_multiplier(4.0);
        let mut orb = MeshInstance3D::new_alloc();
        orb.set_mesh(&super::models::sphere(0.16, 0.32, 12, 6));
        orb.set_material_override(&mat.upcast::<Material>());
        orb.set_cast_shadows_setting(
            godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF,
        );
        node.add_child(&orb);
        node.add_child(&super::models::light(color, 2.5, 5.0));
        let lift = Vector3::new(0.0, 1.2, 0.0);
        node.set_position(from + lift);
        self.root.add_child(&node);
        self.bolts.push(Bolt {
            node,
            from: from + lift,
            to: to + lift,
            t: 0.0,
        });
    }

    /// A crescent blade trail in front of an attacker; vertical for overhead
    /// chops. It appears when the blade actually moves (after `delay`).
    pub fn slash(&mut self, pos: Vector3, yaw: f32, vertical: bool, delay: f32, color: Color) {
        let mut material = StandardMaterial3D::new_gd();
        material.set_shading_mode(ShadingMode::UNSHADED);
        material.set_transparency(Transparency::ALPHA);
        material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        material.set_blend_mode(godot::classes::base_material_3d::BlendMode::ADD);
        material.set_albedo(Color::from_rgba(color.r, color.g, color.b, 0.0));
        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_mesh(&crescent());
        mesh.set_material_override(&material.clone().upcast::<Material>());
        mesh.set_cast_shadows_setting(
            godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF,
        );
        let tilt = if vertical {
            Basis::from_axis_angle(Vector3::FORWARD, std::f32::consts::FRAC_PI_2)
        } else {
            Basis::IDENTITY
        };
        mesh.set_transform(Transform3D::new(
            Basis::from_axis_angle(Vector3::UP, yaw) * tilt,
            pos + Vector3::new(0.0, 1.1, 0.0)
                + Basis::from_axis_angle(Vector3::UP, yaw) * Vector3::new(0.0, 0.0, 0.45),
        ));
        self.root.add_child(&mesh);
        self.slashes.push(Slash {
            mesh,
            material,
            delay,
            t: 0.0,
        });
    }

    /// An expanding ring of the given radius (spell areas, explosions).
    pub fn nova(&mut self, pos: Vector3, radius: f32, color: Color) {
        self.ring_ex(pos, color, radius.max(0.8) * 2.5, 0.45);
    }

    fn ring_ex(&mut self, pos: Vector3, color: Color, size: f32, life: f32) {
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
            size,
            life,
        });
    }

    pub fn update(&mut self, delta: f32) {
        self.slashes.retain_mut(|s| {
            s.t += delta;
            let k = (s.t - s.delay) / 0.22;
            if k >= 1.0 {
                s.mesh.queue_free();
                return false;
            }
            let mut c = s.material.get_albedo();
            c.a = if k < 0.0 { 0.0 } else { 0.9 * (1.0 - k) };
            s.material.set_albedo(c);
            let grow = 0.85 + k.max(0.0) * 0.25;
            let tf = s.mesh.get_transform();
            s.mesh.set_transform(Transform3D::new(
                tf.basis.orthonormalized() * Basis::from_scale(Vector3::splat(grow)),
                tf.origin,
            ));
            true
        });
        self.bolts.retain_mut(|b| {
            b.t += delta;
            let k = (b.t / 0.25).min(1.0);
            let arc = Vector3::new(0.0, (k * std::f32::consts::PI).sin() * 0.5, 0.0);
            b.node.set_position(b.from.lerp(b.to, k) + arc);
            if k >= 1.0 {
                b.node.queue_free();
                return false;
            }
            true
        });
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
            if r.t >= r.life {
                r.mesh.queue_free();
                return false;
            }
            let k = r.t / r.life;
            r.mesh
                .set_scale(Vector3::new(0.4 + k * r.size, 1.0, 0.4 + k * r.size));
            let mut c = r.material.get_albedo();
            c.a = 0.7 * (1.0 - k);
            r.material.set_albedo(c);
            true
        });
    }
}

/// A flat half-ring facing up, opening forward (+Z): the shape of a sword arc.
fn crescent() -> Gd<godot::classes::Mesh> {
    use godot::classes::mesh::PrimitiveType;
    use godot::classes::SurfaceTool;
    let mut st = SurfaceTool::new_gd();
    st.begin(PrimitiveType::TRIANGLES);
    let n = 20;
    for i in 0..n {
        let a0 = -1.3 + 2.6 * i as f32 / n as f32;
        let a1 = -1.3 + 2.6 * (i + 1) as f32 / n as f32;
        let fade = |a: f32| 1.0 - (a / 1.3).abs().powi(2);
        let pt = |a: f32, r: f32| Vector3::new(a.sin() * r, 0.0, a.cos() * r);
        let w0 = 0.12 + 0.35 * fade(a0);
        let w1 = 0.12 + 0.35 * fade(a1);
        let quad = [
            pt(a0, 1.5),
            pt(a1, 1.5),
            pt(a1, 1.5 - w1),
            pt(a0, 1.5),
            pt(a1, 1.5 - w1),
            pt(a0, 1.5 - w0),
        ];
        for v in quad {
            st.set_normal(Vector3::UP);
            st.add_vertex(v);
        }
    }
    st.commit().unwrap().upcast()
}
