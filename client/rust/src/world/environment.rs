//! Sky, ambient light, fog and the sun.

use godot::classes::environment::{AmbientSource, BgMode, ToneMapper};
use godot::classes::light_3d::Param;
use godot::classes::{
    DirectionalLight3D, Environment, Node3D, ProceduralSkyMaterial, Sky, WorldEnvironment,
};
use godot::prelude::*;

pub fn build(parent: &mut Gd<Node3D>) {
    let mut sky_mat = ProceduralSkyMaterial::new_gd();
    sky_mat.set_sky_top_color(Color::from_rgb(0.32, 0.52, 0.85));
    sky_mat.set_sky_horizon_color(Color::from_rgb(0.72, 0.82, 0.92));
    sky_mat.set_ground_horizon_color(Color::from_rgb(0.72, 0.82, 0.92));
    sky_mat.set_ground_bottom_color(Color::from_rgb(0.2, 0.3, 0.45));
    let mut sky = Sky::new_gd();
    sky.set_material(&sky_mat);

    let mut env = Environment::new_gd();
    env.set_background(BgMode::SKY);
    env.set_sky(&sky);
    env.set_ambient_source(AmbientSource::SKY);
    env.set_ambient_light_energy(0.9);
    env.set_tonemapper(ToneMapper::FILMIC);
    env.set_fog_enabled(true);
    env.set_fog_light_color(Color::from_rgb(0.7, 0.8, 0.9));
    env.set_fog_density(0.006);
    env.set_ssao_enabled(true);

    let mut we = WorldEnvironment::new_alloc();
    we.set_environment(&env);
    parent.add_child(&we);

    let mut sun = DirectionalLight3D::new_alloc();
    sun.set_rotation(Vector3::new(-0.9, -0.6, 0.0));
    sun.set_color(Color::from_rgb(1.0, 0.95, 0.85));
    sun.set_param(Param::ENERGY, 1.15);
    sun.set_shadow(true);
    sun.set_param(Param::SHADOW_MAX_DISTANCE, 45.0);
    parent.add_child(&sun);
}
