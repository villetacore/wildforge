//! Procedural combat animation: keyframed poses for attacks, combos, rolls,
//! jumps, blocks, parries and enemy wind-ups, layered over the walk cycle.
//! Durations match the server's action timings (server/internal/game/action.go).

use std::f32::consts::{PI, TAU};

use godot::prelude::*;

/// Joint rotations (Euler YXZ, radians) and root offsets for one frame.
#[derive(Clone, Copy, Default)]
pub struct Pose {
    pub arm_r: Vector3,
    pub arm_l: Vector3,
    pub legs: f32,
    pub weapon: Vector3,
    pub pitch: f32,
    pub twist: f32,
    pub lift: f32,
    pub lean: f32,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Piecewise smooth interpolation through (time, value) keys.
fn kf(t: f32, keys: &[(f32, f32)]) -> f32 {
    if t <= keys[0].0 {
        return keys[0].1;
    }
    for w in keys.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b.0 {
            let k = smooth((t - a.0) / (b.0 - a.0).max(1e-4));
            return a.1 + (b.1 - a.1) * k;
        }
    }
    keys[keys.len() - 1].1
}

/// Total length of an animation; looping poses return f32::MAX.
pub fn duration(anim: &str) -> f32 {
    match anim {
        "light1" | "light2" => 0.42,
        "light3" => 0.62,
        "heavy1" => 0.85,
        "heavy2" => 0.9,
        "finisher" => 1.0,
        "dodge" => 0.5,
        "jump" => 0.62,
        "parry" => 0.45,
        "guardbreak" => 1.0,
        "windup" => 0.6,
        "attack" => 0.4,
        "block" | "stagger" => f32::MAX,
        _ => 0.0,
    }
}

/// Is this an attack that should leave a blade trail?
pub fn is_slash(anim: &str) -> Option<(f32, bool)> {
    // (delay until the blade moves, vertical swing)
    match anim {
        "light1" | "light2" => Some((0.14, false)),
        "light3" => Some((0.26, true)),
        "heavy1" => Some((0.44, true)),
        "heavy2" => Some((0.45, false)),
        "finisher" => Some((0.5, true)),
        _ => None,
    }
}

/// The pose for an animation at time t, or None when it has finished.
pub fn pose(anim: &str, t: f32) -> Option<Pose> {
    if t > duration(anim) {
        return None;
    }
    let mut p = Pose {
        weapon: Vector3::new(0.4, 0.0, 0.0),
        ..Default::default()
    };
    match anim {
        // Horizontal slash, right to left, with a body twist.
        "light1" => {
            p.arm_r = Vector3::new(
                -1.45,
                kf(t, &[(0.0, 0.2), (0.12, 1.3), (0.24, -1.3), (0.42, -0.2)]),
                0.0,
            );
            p.twist = kf(t, &[(0.0, 0.0), (0.12, 0.45), (0.24, -0.5), (0.42, 0.0)]);
            p.weapon = Vector3::new(-0.2, 0.0, 0.0);
            p.arm_l = Vector3::new(-0.4, 0.0, -0.3);
        }
        // Backhand slash, left to right.
        "light2" => {
            p.arm_r = Vector3::new(
                -1.4,
                kf(t, &[(0.0, -0.3), (0.12, -1.3), (0.24, 1.25), (0.42, 0.2)]),
                0.0,
            );
            p.twist = kf(t, &[(0.0, 0.0), (0.12, -0.5), (0.24, 0.5), (0.42, 0.0)]);
            p.weapon = Vector3::new(-0.2, 0.0, 0.0);
            p.arm_l = Vector3::new(-0.4, 0.0, -0.3);
        }
        // Overhead chop that ends the light chain.
        "light3" => {
            p.arm_r = Vector3::new(
                kf(t, &[(0.0, -0.4), (0.22, -3.0), (0.34, -0.5), (0.62, -0.2)]),
                0.0,
                0.0,
            );
            p.arm_l = Vector3::new(
                kf(t, &[(0.0, -0.4), (0.22, -2.8), (0.34, -0.6), (0.62, -0.2)]),
                0.0,
                0.0,
            );
            p.lean = kf(t, &[(0.0, 0.0), (0.22, -0.15), (0.34, 0.3), (0.62, 0.0)]);
            p.lift = kf(t, &[(0.0, 0.0), (0.24, 0.25), (0.34, 0.0)]);
            p.legs = kf(t, &[(0.2, 0.0), (0.3, 0.5), (0.5, 0.0)]);
        }
        // Two-handed diagonal slam with a long wind-up.
        "heavy1" => {
            p.arm_r = Vector3::new(
                kf(t, &[(0.0, -0.3), (0.42, -2.7), (0.56, -0.3), (0.85, -0.2)]),
                kf(t, &[(0.0, 0.0), (0.42, 1.0), (0.56, -0.8), (0.85, 0.0)]),
                0.0,
            );
            p.arm_l = Vector3::new(kf(t, &[(0.0, -0.3), (0.42, -2.4), (0.56, -0.4)]), -0.3, 0.0);
            p.twist = kf(t, &[(0.0, 0.0), (0.42, 0.7), (0.56, -0.5), (0.85, 0.0)]);
            p.lean = kf(t, &[(0.0, 0.0), (0.42, -0.2), (0.56, 0.4), (0.85, 0.0)]);
            p.weapon = Vector3::new(-0.4, 0.0, 0.0);
        }
        // Whirlwind: a full spin with the blade held out.
        "heavy2" => {
            p.arm_r = Vector3::new(
                -1.55,
                kf(t, &[(0.0, 0.0), (0.35, 1.2), (0.6, -1.0), (0.9, 0.0)]),
                -0.2,
            );
            p.arm_l = Vector3::new(-0.3, 0.0, -1.2);
            p.twist = kf(
                t,
                &[(0.0, 0.0), (0.35, 0.8), (0.65, 0.8 - TAU), (0.9, -TAU)],
            );
            p.weapon = Vector3::new(-0.25, 0.0, 0.0);
            p.lean = 0.15;
        }
        // Leaping slam: up, then down onto the target.
        "finisher" => {
            p.lift = kf(t, &[(0.0, 0.0), (0.15, 0.0), (0.38, 1.1), (0.55, 0.0)]);
            p.arm_r = Vector3::new(
                kf(t, &[(0.0, -0.4), (0.38, -3.1), (0.55, -0.6), (1.0, -0.2)]),
                0.0,
                0.0,
            );
            p.arm_l = Vector3::new(kf(t, &[(0.0, -0.4), (0.38, -2.9), (0.55, -0.6)]), 0.0, 0.0);
            p.lean = kf(t, &[(0.0, 0.0), (0.38, -0.3), (0.55, 0.55), (1.0, 0.0)]);
            p.legs = kf(t, &[(0.15, 0.0), (0.3, -0.9), (0.55, 0.4), (0.8, 0.0)]);
            p.weapon = Vector3::new(-0.5, 0.0, 0.0);
        }
        // Forward roll.
        "dodge" => {
            p.pitch = kf(t, &[(0.0, 0.0), (0.42, TAU)]);
            p.lift = kf(t, &[(0.0, 0.0), (0.1, -0.35), (0.35, -0.35), (0.45, 0.0)]);
            p.arm_r = Vector3::new(-1.6, 0.0, 0.0);
            p.arm_l = Vector3::new(-1.6, 0.0, 0.0);
            p.legs = -1.2 * (1.0 - smooth((t - 0.35) / 0.15));
        }
        "jump" => {
            let k = (t / 0.62).clamp(0.0, 1.0);
            p.lift = (k * PI).sin() * 1.0;
            p.legs = -0.9 * (k * PI).sin();
            p.arm_r = Vector3::new(-0.9 * (k * PI).sin(), 0.0, 0.4);
            p.arm_l = Vector3::new(-0.9 * (k * PI).sin(), 0.0, -0.4);
        }
        // Blade held across the body.
        "block" => {
            p.arm_r = Vector3::new(-1.25, -0.7, 0.0);
            p.arm_l = Vector3::new(-1.1, 0.5, 0.0);
            p.weapon = Vector3::new(0.0, 0.0, 1.45);
            p.lean = -0.08;
        }
        "parry" => {
            p.arm_r = Vector3::new(-1.3, kf(t, &[(0.0, -0.7), (0.12, 0.9), (0.45, 0.1)]), 0.0);
            p.weapon = Vector3::new(0.0, 0.0, kf(t, &[(0.0, 1.45), (0.12, 0.2)]));
            p.twist = kf(t, &[(0.0, 0.0), (0.12, 0.4), (0.45, 0.0)]);
        }
        "guardbreak" | "stagger" => {
            let w = (t * 9.0).sin() * 0.08;
            p.lean = -0.3 + w;
            p.arm_r = Vector3::new(0.3, 0.0, 0.8);
            p.arm_l = Vector3::new(0.3, 0.0, -0.8);
            p.twist = w * 2.0;
        }
        // Enemy telegraph: rear back with arms raised.
        "windup" => {
            let k = smooth(t / 0.45);
            p.lean = -0.35 * k;
            p.pitch = -0.25 * k;
            p.arm_r = Vector3::new(-2.6 * k, 0.4 * k, 0.0);
            p.arm_l = Vector3::new(-2.4 * k, -0.4 * k, 0.0);
            p.lift = 0.1 * k;
        }
        "attack" => {
            let k = 1.0 - smooth(t / 0.4);
            p.lean = 0.45 * k;
            p.pitch = 0.3 * k;
            p.arm_r = Vector3::new(-0.9 * k, 0.0, 0.0);
            p.arm_l = Vector3::new(-0.9 * k, 0.0, 0.0);
        }
        _ => return None,
    }
    Some(p)
}
