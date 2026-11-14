//! Cinematic post-process: split toning (warm highlights, cool shadows), a
//! filmic contrast curve, gentle desaturation, vignette and film grain.

use godot::classes::control::{LayoutPreset, MouseFilter};
use godot::classes::{CanvasLayer, ColorRect, Node3D, Shader, ShaderMaterial};
use godot::prelude::*;

const GRADE_SHADER: &str = r#"
shader_type canvas_item;
uniform sampler2D screen : hint_screen_texture, filter_linear_mipmap;
uniform float strength = 1.0;
uniform float night = 0.0;
float luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }
void fragment() {
    vec3 c = texture(screen, SCREEN_UV).rgb;
    float l = luma(c);
    // Desaturate a little and pull towards a muted, weathered palette.
    c = mix(vec3(l), c, mix(0.86, 0.78, night));
    // Split toning.
    vec3 shadows = mix(vec3(0.92, 0.97, 1.08), vec3(0.85, 0.92, 1.15), night);
    vec3 highlights = vec3(1.07, 1.0, 0.9);
    c *= mix(shadows, highlights, smoothstep(0.15, 0.75, l));
    // Filmic S-curve.
    c = clamp(c, 0.0, 1.0);
    c = c * c * (3.0 - 2.0 * c) * 0.35 + c * 0.65;
    // Vignette.
    vec2 d = SCREEN_UV - 0.5;
    float v = smoothstep(0.85, 0.25, length(d * vec2(1.25, 1.0)));
    c *= mix(0.62, 1.0, v);
    // Grain.
    float g = hash(SCREEN_UV * 1000.0 + fract(TIME) * 17.0) - 0.5;
    c += g * 0.025;
    COLOR = vec4(mix(texture(screen, SCREEN_UV).rgb, c, strength), 1.0);
}
"#;

pub struct Grade {
    material: Gd<ShaderMaterial>,
    rect: Gd<ColorRect>,
}

impl Grade {
    pub fn build(parent: &mut Gd<Node3D>) -> Grade {
        let mut layer = CanvasLayer::new_alloc();
        layer.set_layer(0);
        let mut shader = Shader::new_gd();
        shader.set_code(GRADE_SHADER);
        let mut material = ShaderMaterial::new_gd();
        material.set_shader(&shader);
        let mut rect = ColorRect::new_alloc();
        rect.set_anchors_preset(LayoutPreset::FULL_RECT);
        rect.set_mouse_filter(MouseFilter::IGNORE);
        rect.set_material(&material);
        layer.add_child(&rect);
        parent.add_child(&layer);
        Grade { material, rect }
    }

    pub fn update(&mut self, night: f32) {
        self.material
            .set_shader_parameter("night", &night.to_variant());
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.rect.set_visible(on);
    }
}
