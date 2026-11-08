//! Follow camera looking down at the player from a fixed angle, with zoom.

use godot::classes::{Camera3D, Node3D};
use godot::prelude::*;

const PITCH: f32 = 0.95; // radians above the horizon
const MIN_DIST: f32 = 7.0;
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
        cam.set_fov(50.0);
        cam.set_far(220.0);
        parent.add_child(&cam);
        cam.make_current();
        CameraRig {
            cam,
            focus: Vector3::ZERO,
            distance: 16.0,
            target_distance: 16.0,
            snapped: false,
        }
    }

    pub fn zoom(&mut self, steps: f32) {
        self.target_distance =
            (self.target_distance * 1.12f32.powf(steps)).clamp(MIN_DIST, MAX_DIST);
    }

    pub fn update(&mut self, delta: f32, target: Vector3) {
        if !self.snapped {
            self.focus = target;
            self.snapped = true;
        }
        self.focus = self.focus.lerp(target, 1.0 - (-delta * 8.0).exp());
        self.distance += (self.target_distance - self.distance) * (1.0 - (-delta * 10.0).exp());
        let look = self.focus + Vector3::new(0.0, 0.8, 0.0);
        let offset = Vector3::new(0.0, PITCH.sin(), PITCH.cos()) * self.distance;
        self.cam.look_at_from_position(look + offset, look);
    }
}
