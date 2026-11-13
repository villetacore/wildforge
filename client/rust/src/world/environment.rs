//! Sky, ambient light, fog and the sun.

use godot::classes::environment::{AmbientSource, BgMode, ToneMapper};
use godot::classes::light_3d::Param;
use godot::classes::{
    DirectionalLight3D, Environment, Node3D, ProceduralSkyMaterial, Sky, WorldEnvironment,
};
use godot::prelude::*;

pub fn build(parent: &mut Gd<Node3D>) -> Lighting {
    let mut sky_mat = ProceduralSkyMaterial::new_gd();
    sky_mat.set_sky_top_color(Color::from_rgb(0.28, 0.32, 0.40));
    sky_mat.set_sky_horizon_color(Color::from_rgb(0.55, 0.57, 0.60));
    sky_mat.set_ground_horizon_color(Color::from_rgb(0.55, 0.57, 0.60));
    sky_mat.set_ground_bottom_color(Color::from_rgb(0.2, 0.3, 0.45));
    let mut sky = Sky::new_gd();
    sky.set_material(&sky_mat);

    let mut env = Environment::new_gd();
    env.set_background(BgMode::SKY);
    env.set_sky(&sky);
    env.set_ambient_source(AmbientSource::COLOR);
    env.set_ambient_light_color(Color::from_rgb(0.55, 0.60, 0.70));
    env.set_ambient_light_energy(0.65);
    env.set_tonemapper(ToneMapper::FILMIC);
    env.set_fog_enabled(true);
    env.set_fog_light_color(Color::from_rgb(0.72, 0.75, 0.80));
    env.set_fog_density(0.006);
    env.set_ssao_enabled(true);

    let mut we = WorldEnvironment::new_alloc();
    we.set_environment(&env);
    parent.add_child(&we);

    let mut sun = DirectionalLight3D::new_alloc();
    sun.set_rotation(Vector3::new(-0.9, -0.6, 0.0));
    sun.set_color(Color::from_rgb(0.95, 0.90, 0.80));
    sun.set_param(Param::ENERGY, 0.85);
    sun.set_shadow(true);
    sun.set_param(Param::SHADOW_MAX_DISTANCE, 45.0);
    parent.add_child(&sun);
    Lighting {
        env,
        sun,
        underground: false,
    }
}

pub struct Lighting {
    env: Gd<Environment>,
    sun: Gd<DirectionalLight3D>,
    underground: bool,
}

impl Lighting {
    pub fn update(&mut self, underground: bool) {
        if self.underground == underground {
            return;
        }
        self.underground = underground;
        self.sun.set_visible(!underground);
        self.env.set_background(if underground {
            BgMode::COLOR
        } else {
            BgMode::SKY
        });
        self.env.set_bg_color(Color::from_rgb(0.015, 0.018, 0.025));
        self.env
            .set_ambient_light_energy(if underground { 0.22 } else { 0.65 });
        self.env.set_fog_light_color(if underground {
            Color::from_rgb(0.025, 0.03, 0.04)
        } else {
            Color::from_rgb(0.72, 0.75, 0.80)
        });
        self.env
            .set_fog_density(if underground { 0.045 } else { 0.006 });
    }
}
