//! Sky, sun and moon with a day/night cycle, zone-tinted fog and ambient
//! light, bloom and ambient occlusion.

use std::f32::consts::{PI, TAU};

use godot::classes::directional_light_3d::ShadowMode;
use godot::classes::environment::{AmbientSource, BgMode, GlowBlendMode, ToneMapper};
use godot::classes::light_3d::BakeMode;
use godot::classes::light_3d::Param;
use godot::classes::{
    DirectionalLight3D, Environment, Node3D, ProceduralSkyMaterial, Sky, WorldEnvironment,
};
use godot::prelude::*;

pub fn build(parent: &mut Gd<Node3D>) -> Lighting {
    let mut sky_mat = ProceduralSkyMaterial::new_gd();
    sky_mat.set_sun_angle_max(25.0);
    sky_mat.set_sky_curve(0.12);
    let mut sky = Sky::new_gd();
    sky.set_material(&sky_mat);

    let mut env = Environment::new_gd();
    env.set_background(BgMode::SKY);
    env.set_sky(&sky);
    env.set_ambient_source(AmbientSource::COLOR);
    env.set_tonemapper(ToneMapper::ACES);
    env.set_tonemap_exposure(1.05);
    env.set_fog_enabled(true);
    env.set_fog_sky_affect(0.35);
    env.set_ssao_enabled(true);
    env.set_ssao_radius(1.2);
    env.set_ssao_intensity(1.6);
    env.set_glow_enabled(true);
    env.set_glow_intensity(0.7);
    env.set_glow_bloom(0.06);
    env.set_glow_hdr_bleed_threshold(0.9);
    env.set_glow_blend_mode(GlowBlendMode::SOFTLIGHT);
    env.set_adjustment_enabled(true);
    env.set_adjustment_saturation(1.0);
    env.set_adjustment_contrast(1.08);
    env.set_volumetric_fog_enabled(true);
    env.set_volumetric_fog_density(0.006);
    env.set_volumetric_fog_anisotropy(0.6);
    env.set_volumetric_fog_length(80.0);

    let mut we = WorldEnvironment::new_alloc();
    we.set_environment(&env);
    parent.add_child(&we);

    let mut sun = DirectionalLight3D::new_alloc();
    sun.set_shadow(true);
    sun.set_param(Param::SHADOW_MAX_DISTANCE, 60.0);
    sun.set_shadow_mode(ShadowMode::PARALLEL_4_SPLITS);
    sun.set_param(Param::SHADOW_BLUR, 1.5);
    sun.set_bake_mode(BakeMode::DISABLED);
    parent.add_child(&sun);

    let mut moon = DirectionalLight3D::new_alloc();
    moon.set_color(Color::from_rgb(0.55, 0.65, 1.0));
    moon.set_shadow(true);
    moon.set_param(Param::SHADOW_MAX_DISTANCE, 40.0);
    parent.add_child(&moon);

    Lighting {
        env,
        sky: sky_mat,
        sun,
        moon,
        fog: Color::from_rgb(0.72, 0.75, 0.80),
        ambient: Color::from_rgb(0.55, 0.6, 0.7),
        night: 0.0,
    }
}

pub struct Lighting {
    env: Gd<Environment>,
    sky: Gd<ProceduralSkyMaterial>,
    sun: Gd<DirectionalLight3D>,
    moon: Gd<DirectionalLight3D>,
    fog: Color,
    ambient: Color,
    pub night: f32,
}

fn lerp_c(a: Color, b: Color, t: f32) -> Color {
    a.lerp(b, t.clamp(0.0, 1.0) as f64)
}

impl Lighting {
    /// tod: 0..1 (0.5 = noon). zone_fog/zone_ambient tint the air.
    pub fn update(
        &mut self,
        delta: f32,
        tod: f32,
        underground: bool,
        zone_fog: Color,
        zone_ambient: Color,
        fog_density: f32,
    ) {
        let k = 1.0 - (-delta * 1.5).exp();
        self.fog = lerp_c(self.fog, zone_fog, k);
        self.ambient = lerp_c(self.ambient, zone_ambient, k);
        // Sun elevation: -1 at midnight, +1 at noon.
        let angle = (tod - 0.25) * TAU;
        let elevation = angle.sin();
        let day = ((elevation + 0.12) / 0.4).clamp(0.0, 1.0);
        let dusk = (1.0 - (elevation.abs() / 0.3)).clamp(0.0, 1.0);
        self.night = 1.0 - day;
        if underground {
            self.sun.set_visible(false);
            self.moon.set_visible(false);
            self.env.set_background(BgMode::COLOR);
            self.env.set_bg_color(Color::from_rgb(0.01, 0.012, 0.018));
            self.env
                .set_ambient_light_color(Color::from_rgb(0.55, 0.55, 0.68));
            self.env.set_ambient_light_energy(0.5);
            self.env
                .set_fog_light_color(Color::from_rgb(0.03, 0.035, 0.05));
            self.env.set_fog_density(0.03);
            self.night = 1.0;
            return;
        }
        self.env.set_background(BgMode::SKY);
        // Sun travels east to west, tilted south.
        self.sun.set_visible(day > 0.0);
        self.sun.set_rotation(Vector3::new(
            -(elevation.max(0.05)).asin().max(0.08),
            angle + PI * 0.5,
            0.0,
        ));
        let warm = Color::from_rgb(1.0, 0.62, 0.38);
        let noon = Color::from_rgb(1.0, 0.95, 0.86);
        self.sun.set_color(lerp_c(noon, warm, dusk));
        self.sun.set_param(Param::ENERGY, 1.25 * day);
        self.moon.set_visible(self.night > 0.2);
        self.moon
            .set_rotation(Vector3::new(-0.9, angle - PI * 0.5, 0.0));
        self.moon.set_param(Param::ENERGY, 0.22 * self.night);

        let night_sky = Color::from_rgb(0.02, 0.03, 0.08);
        let day_top = Color::from_rgb(0.24, 0.42, 0.72);
        let day_horizon = lerp_c(Color::from_rgb(0.68, 0.76, 0.86), self.fog, 0.4);
        let dusk_horizon = Color::from_rgb(0.95, 0.55, 0.35);
        let top = lerp_c(night_sky, day_top, day);
        let horizon = lerp_c(
            lerp_c(Color::from_rgb(0.06, 0.07, 0.14), day_horizon, day),
            dusk_horizon,
            dusk * 0.7,
        );
        self.sky.set_sky_top_color(top);
        self.sky.set_sky_horizon_color(horizon);
        self.sky.set_ground_horizon_color(horizon);
        self.sky.set_ground_bottom_color(lerp_c(
            Color::from_rgb(0.02, 0.03, 0.05),
            Color::from_rgb(0.18, 0.25, 0.35),
            day,
        ));
        self.sky.set_sun_curve(0.08);

        let amb = lerp_c(Color::from_rgb(0.25, 0.3, 0.5), self.ambient, day);
        self.env.set_ambient_light_color(amb);
        self.env.set_ambient_light_energy(0.32 + 0.4 * day);
        let fog = lerp_c(
            Color::from_rgb(0.05, 0.06, 0.1),
            lerp_c(self.fog, dusk_horizon, dusk * 0.4),
            day,
        );
        self.env.set_fog_light_color(fog);
        self.env
            .set_fog_density(fog_density * (1.0 + self.night * 0.6));
    }
}
