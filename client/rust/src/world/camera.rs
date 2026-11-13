//! Follow camera with a low third-person view that rises as the player zooms out.

use godot::classes::{Camera3D, Node3D};
use godot::prelude::*;

const MIN_DIST: f32 = 4.5;
const MAX_DIST: f32 = 32.0;

pub struct CameraRig {
    cam: Gd<Camera3D>,
    focus: Vector3,
    distance: f32,
    target_distance: f32,
    snapped: bool,
}

impl CameraRig {
    pub fn new(parent: &mut Gd<Node3D>) -> CameraRig {
        let mut cam = Camera3D::new_alloc();
        cam.set_fov(58.0);
        cam.set_far(220.0);
        parent.add_child(&cam);
        cam.make_current();
        CameraRig {
            cam,
            focus: Vector3::ZERO,
            distance: 8.5,
            target_distance: 8.5,
            snapped: false,
        }
    }

    pub fn zoom(&mut self, steps: f32) {
        self.target_distance =
            (self.target_distance * 1.12f32.powf(steps)).clamp(MIN_DIST, MAX_DIST);
    }

    pub fn update(&mut self, delta: f32, target: Vector3) {
        if !self.snapped || self.focus.distance_to(target) > 20.0 {
            self.focus = target;
            self.snapped = true;
        }
        self.focus = self.focus.lerp(target, 1.0 - (-delta * 8.0).exp());
        self.distance += (self.target_distance - self.distance) * (1.0 - (-delta * 10.0).exp());
        let look = self.focus + Vector3::new(0.0, 1.4, 0.0);
        let zoom = (self.distance - MIN_DIST) / (MAX_DIST - MIN_DIST);
        let underground = target.y < -5.0;
        let pitch = if underground {
            0.95
        } else {
            0.26 + 0.65 * zoom
        };
        let view_distance = if underground {
            self.distance.min(4.5)
        } else {
            self.distance
        };
        let offset = Vector3::new(0.0, pitch.sin(), pitch.cos()) * view_distance;
        self.cam.look_at_from_position(look + offset, look);
    }
}
