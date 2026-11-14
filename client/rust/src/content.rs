//! Client-side content: finds the shared `content/` folder, loads textures
//! from it and caches materials and item icons built from them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use godot::classes::base_material_3d::{Feature, Flags, TextureFilter, TextureParam};
use godot::classes::{Image, ImageTexture, Os, ProjectSettings, StandardMaterial3D, Texture2D};
use godot::prelude::*;

/// Parses "#rrggbb" (or "rrggbb") into a color; magenta on error.
pub fn parse_color(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0xFF00FF);
    Color::from_rgb(
        ((v >> 16) & 0xFF) as f32 / 255.0,
        ((v >> 8) & 0xFF) as f32 / 255.0,
        (v & 0xFF) as f32 / 255.0,
    )
}

/// Locates the content directory: $WILDFORGE_CONTENT, then the repository
/// layout (client/godot/../../content), then `content/` next to the binary.
pub fn find_content_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(dir) = std::env::var("WILDFORGE_CONTENT") {
        candidates.push(PathBuf::from(dir));
    }
    let res = ProjectSettings::singleton()
        .globalize_path("res://")
        .to_string();
    if !res.is_empty() {
        candidates.push(Path::new(&res).join("../../content"));
    }
    let exe = Os::singleton().get_executable_path().to_string();
    if let Some(dir) = Path::new(&exe).parent() {
        candidates.push(dir.join("content"));
        candidates.push(dir.join("../Resources/content")); // macOS .app bundle
    }
    candidates.into_iter().find(|d| d.join("textures").is_dir())
}

pub struct ContentLibrary {
    dir: Option<PathBuf>,
    textures: RefCell<HashMap<String, Option<Gd<Texture2D>>>>,
    normals: RefCell<HashMap<String, Option<Gd<Texture2D>>>>,
    materials: RefCell<HashMap<String, Gd<StandardMaterial3D>>>,
    icons: RefCell<HashMap<String, Gd<Texture2D>>>,
}

impl ContentLibrary {
    pub fn new() -> ContentLibrary {
        let dir = find_content_dir();
        match &dir {
            Some(d) => godot_print!("wildforge: content from {}", d.display()),
            None => {
                godot_warn!("wildforge: content/ folder not found, using flat colors (set WILDFORGE_CONTENT)");
            }
        }
        ContentLibrary {
            dir,
            textures: RefCell::default(),
            normals: RefCell::default(),
            materials: RefCell::default(),
            icons: RefCell::default(),
        }
    }

    /// Loads content/textures/<name>.png (cached). None if missing.
    pub fn texture(&self, name: &str) -> Option<Gd<Texture2D>> {
        if let Some(t) = self.textures.borrow().get(name) {
            return t.clone();
        }
        let tex = self.dir.as_ref().and_then(|d| {
            let path = d.join("textures").join(format!("{name}.png"));
            let mut img = Image::load_from_file(path.to_string_lossy().as_ref())?;
            img.generate_mipmaps();
            ImageTexture::create_from_image(&img).map(|t| t.upcast::<Texture2D>())
        });
        self.textures
            .borrow_mut()
            .insert(name.to_string(), tex.clone());
        tex
    }

    /// A normal map derived from the texture's brightness (cached).
    pub fn normal_map(&self, name: &str) -> Option<Gd<Texture2D>> {
        if let Some(t) = self.normals.borrow().get(name) {
            return t.clone();
        }
        let tex = self.dir.as_ref().and_then(|d| {
            let path = d.join("textures").join(format!("{name}.png"));
            let mut img = Image::load_from_file(path.to_string_lossy().as_ref())?;
            img.bump_map_to_normal_map_ex().bump_scale(3.0).done();
            img.generate_mipmaps();
            ImageTexture::create_from_image(&img).map(|t| t.upcast::<Texture2D>())
        });
        self.normals
            .borrow_mut()
            .insert(name.to_string(), tex.clone());
        tex
    }

    /// A lit material with an optional texture multiplied by `tint`.
    /// `uv_scale` repeats the texture; `vertex_color` multiplies by mesh colors.
    pub fn material(
        &self,
        texture: &str,
        tint: Color,
        uv_scale: f32,
        vertex_color: bool,
    ) -> Gd<StandardMaterial3D> {
        let key = format!("{texture}|{}|{uv_scale}|{vertex_color}", tint.to_html());
        if let Some(m) = self.materials.borrow().get(&key) {
            return m.clone();
        }
        let mut m = StandardMaterial3D::new_gd();
        m.set_albedo(tint);
        m.set_roughness(match texture {
            "metal" | "rusted_iron" => 0.45,
            "steel" => 0.32,
            "slime" | "crystal" | "ice" => 0.15,
            "marble" => 0.35,
            "water" => 0.1,
            _ => 0.88,
        });
        if texture == "metal" || texture == "steel" {
            m.set_metallic(if texture == "steel" { 0.75 } else { 0.55 });
            m.set_specular(0.7);
        }
        m.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS_ANISOTROPIC);
        if let Some(tex) = self.texture(texture) {
            m.set_texture(TextureParam::ALBEDO, &tex);
            let bumpy = !matches!(
                texture,
                "slime" | "crystal" | "water" | "skin" | "window" | "lava"
            );
            if bumpy {
                if let Some(n) = self.normal_map(texture) {
                    m.set_feature(Feature::NORMAL_MAPPING, true);
                    m.set_texture(TextureParam::NORMAL, &n);
                    m.set_normal_scale(match texture {
                        "cobble" | "stone_blocks" | "rocky" | "stone" | "bark" | "roof_tiles"
                        | "slate" => 1.4,
                        _ => 0.8,
                    });
                }
            }
        }
        if uv_scale != 1.0 {
            m.set_uv1_scale(Vector3::new(uv_scale, uv_scale, uv_scale));
        }
        if vertex_color {
            m.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        }
        self.materials.borrow_mut().insert(key, m.clone());
        m
    }

    /// Like `material`, but textured with world-space triplanar mapping so
    /// large walls tile evenly instead of stretching one texture.
    pub fn material_world(&self, texture: &str, tint: Color, scale: f32) -> Gd<StandardMaterial3D> {
        let key = format!("world|{texture}|{}|{scale}", tint.to_html());
        if let Some(m) = self.materials.borrow().get(&key) {
            return m.clone();
        }
        let mut m = self
            .material(texture, tint, 1.0, false)
            .duplicate_resource();
        m.set_flag(Flags::UV1_USE_TRIPLANAR, true);
        m.set_flag(Flags::UV1_USE_WORLD_TRIPLANAR, true);
        m.set_uv1_triplanar_blend_sharpness(4.0);
        m.set_uv1_scale(Vector3::splat(scale));
        self.materials.borrow_mut().insert(key, m.clone());
        m
    }

    /// A small generated icon for an inventory item, shaped by item type.
    pub fn icon(&self, item_id: &str, kind: &str, color: Color) -> Gd<Texture2D> {
        if let Some(t) = self.icons.borrow().get(item_id) {
            return t.clone();
        }
        const S: i32 = 20;
        let mut img =
            Image::create_empty(S, S, false, godot::classes::image::Format::RGBA8).unwrap();
        let dark = color.darkened(0.5);
        for y in 0..S {
            for x in 0..S {
                let (fx, fy) = (x as f32 - 9.5, y as f32 - 9.5);
                let inside = match kind {
                    "potion" => {
                        fx * fx + (fy - 2.0) * (fy - 2.0) < 42.0
                            || (fx.abs() < 2.5 && fy < -2.0 && fy > -8.5)
                    }
                    "weapon" => (fx + fy).abs() < 2.5 && fx > -8.5 && fx < 8.5,
                    "armor" => fx.abs() < 7.5 - (fy.max(0.0) * 0.5) && fy > -8.0 && fy < 8.5,
                    "tool" => {
                        (fy < -4.0 && fy > -8.5 && fx.abs() < 8.0)
                            || (fx.abs() < 1.8 && (-4.0..9.0).contains(&fy))
                    }
                    _ => fx.abs() < 7.0 && fy.abs() < 7.0 && (fx.abs() + fy.abs()) < 12.0,
                };
                let edge = inside && {
                    let near = |dx: f32, dy: f32| {
                        let (gx, gy) = (fx + dx, fy + dy);
                        match kind {
                            "potion" => {
                                gx * gx + (gy - 2.0) * (gy - 2.0) < 42.0
                                    || (gx.abs() < 2.5 && gy < -2.0 && gy > -8.5)
                            }
                            "weapon" => (gx + gy).abs() < 2.5 && gx > -8.5 && gx < 8.5,
                            "armor" => {
                                gx.abs() < 7.5 - (gy.max(0.0) * 0.5) && gy > -8.0 && gy < 8.5
                            }
                            "tool" => {
                                (gy < -4.0 && gy > -8.5 && gx.abs() < 8.0)
                                    || (gx.abs() < 1.8 && (-4.0..9.0).contains(&gy))
                            }
                            _ => gx.abs() < 7.0 && gy.abs() < 7.0 && (gx.abs() + gy.abs()) < 12.0,
                        }
                    };
                    !(near(1.0, 0.0) && near(-1.0, 0.0) && near(0.0, 1.0) && near(0.0, -1.0))
                };
                let c = if edge {
                    dark
                } else if inside {
                    color.lightened(((-fx - fy) / 30.0).max(0.0) as f64)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.0)
                };
                img.set_pixel(x, y, c);
            }
        }
        let tex = ImageTexture::create_from_image(&img)
            .unwrap()
            .upcast::<Texture2D>();
        self.icons
            .borrow_mut()
            .insert(item_id.to_string(), tex.clone());
        tex
    }
}
