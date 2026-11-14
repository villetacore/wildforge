//! Orbit camera: right mouse drag rotates around the player, the wheel zooms,
//! and the view stays above the terrain.

use godot::classes::{Camera3D, Node3D};
use godot::prelude::*;

use super::terrain;

const MIN_DIST: f32 = 3.0;
const MAX_DIST: f32 = 34.0;
const MIN_PITCH: f32 = -0.15;
const MAX_PITCH: f32 = 1.35;

pub struct CameraRig {
    pub cam: Gd<Camera3D>,
    focus: Vector3,
    distance: f32,
    target_distance: f32,
    pub yaw: f32,
    pitch: f32,
    target_yaw: f32,
    target_pitch: f32,
    smoothed_pitch: f32,
    snapped: bool,
    trauma: f32,
    t: f32,
}

impl CameraRig {
    pub fn new(parent: &mut Gd<Node3D>) -> CameraRig {
        let mut cam = Camera3D::new_alloc();
        cam.set_fov(60.0);
        cam.set_near(0.1);
        cam.set_far(260.0);
        parent.add_child(&cam);
        cam.make_current();
        CameraRig {
            cam,
            focus: Vector3::ZERO,
            distance: 9.0,
            target_distance: 9.0,
            yaw: 0.0,
            pitch: 0.45,
            target_yaw: 0.0,
            target_pitch: 0.45,
            smoothed_pitch: 0.45,
            snapped: false,
            trauma: 0.0,
            t: 0.0,
        }
    }

    pub fn zoom(&mut self, steps: f32) {
        self.target_distance =
            (self.target_distance * 1.12f32.powf(steps)).clamp(MIN_DIST, MAX_DIST);
    }

    /// Mouse drag in pixels.
    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.target_yaw -= dx * 0.006;
        self.target_pitch = (self.target_pitch + dy * 0.005).clamp(MIN_PITCH, MAX_PITCH);
    }

    /// Adds screen shake (0..1).
    pub fn shake(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    /// Sets a view parameter directly (used by scripted screenshots).
    pub fn set_view(&mut self, what: &str, v: f32) {
        match what {
            "yaw" => {
                self.target_yaw = v;
                self.yaw = v;
            }
            "pitch" => {
                self.target_pitch = v;
                self.pitch = v;
                self.smoothed_pitch = v;
            }
            _ => {
                self.target_distance = v;
                self.distance = v;
            }
        }
    }

    /// Horizontal forward and right vectors of the view, as (x, z).
    pub fn basis_2d(&self) -> (Vector2, Vector2) {
        let f = Vector2::new(-self.yaw.sin(), -self.yaw.cos());
        let r = Vector2::new(self.yaw.cos(), -self.yaw.sin());
        (f, r)
    }

    pub fn update(&mut self, delta: f32, target: Vector3, blocked: &dyn Fn(f32, f32) -> bool) {
        if !self.snapped || self.focus.distance_to(target) > 20.0 {
            self.focus = target;
            self.snapped = true;
        }
        let k = 1.0 - (-delta * 14.0).exp();
        self.focus = self.focus.lerp(target, 1.0 - (-delta * 10.0).exp());
        self.yaw += (self.target_yaw - self.yaw) * k;
        self.pitch += (self.target_pitch - self.pitch) * k;
        self.distance += (self.target_distance - self.distance) * (1.0 - (-delta * 10.0).exp());
        let underground = target.y < -5.0;
        let dist = if underground {
            self.distance.min(7.0)
        } else {
            self.distance
        };
        let look = self.focus + Vector3::new(0.0, 1.5, 0.0);
        let pitch = if underground {
            self.pitch.max(0.5)
        } else {
            self.pitch
        };
        // Avoid buildings: first look from higher above, then pull in.
        let hits = |pitch: f32, dist: f32| -> Option<f32> {
            let dir = Vector3::new(
                self.yaw.sin() * pitch.cos(),
                pitch.sin(),
                self.yaw.cos() * pitch.cos(),
            );
            let mut t = 1.4;
            while t < dist {
                let p = look + dir * t;
                if blocked(p.x, p.z) && p.y < terrain::height(p.x, p.z) + 11.0 {
                    return Some(t);
                }
                t += 0.25;
            }
            None
        };
        let mut pitch = pitch;
        let mut dist = dist;
        if !underground {
            if let Some(t) = hits(pitch, dist) {
                let mut solved = false;
                for p in [0.75f32, 0.95, 1.15, 1.3] {
                    if p > pitch && hits(p, dist).is_none() {
                        pitch = p;
                        solved = true;
                        break;
                    }
                }
                if !solved {
                    dist = (t - 0.4).max(MIN_DIST);
                }
            }
        }
        self.smoothed_pitch += (pitch - self.smoothed_pitch) * (1.0 - (-delta * 6.0).exp());
        let pitch = self.smoothed_pitch;
        let dir = Vector3::new(
            self.yaw.sin() * pitch.cos(),
            pitch.sin(),
            self.yaw.cos() * pitch.cos(),
        );
        let offset = dir * dist;
        let mut pos = look + offset;
        // Keep the camera above ground.
        let ground = terrain::height(pos.x, pos.z) + 0.6;
        if pos.y < ground && !underground {
            pos.y = ground;
        }
        self.t += delta;
        self.trauma = (self.trauma - delta * 2.2).max(0.0);
        let shake = self.trauma * self.trauma * 0.35;
        let jitter = Vector3::new(
            (self.t * 47.0).sin(),
            (self.t * 53.0).cos(),
            (self.t * 41.0).sin(),
        ) * shake;
        self.cam
            .look_at_from_position(pos + jitter, look + jitter * 0.5);
    }
}
