//! Skeletal characters from glTF (content/models/characters, KayKit CC0).
//! Each model carries its own rig, hand-held weapons and dozens of clips; this
//! module instantiates them, shows the right weapon and maps our combat
//! actions (light1, heavy2, dodge, block...) onto their animations.

use godot::classes::animation::LoopMode;
use godot::classes::base_material_3d::Feature;
use godot::classes::{
    AnimationPlayer, ImporterMeshInstance3D, Material, MeshInstance3D, Node, Node3D,
    StandardMaterial3D,
};
use godot::prelude::*;

use super::models::ActorModel;
use crate::content::ContentLibrary;

/// KayKit characters are about 2.4 units tall; ours are 1.9.
const RIG_SCALE: f32 = 0.78;

const LOOPING: [&str; 9] = [
    "Idle",
    "Walking_A",
    "Running_A",
    "Blocking",
    "2H_Melee_Idle",
    "Unarmed_Idle",
    "Spellcasting",
    "Sit_Chair_Idle",
    "Hit_B",
];

fn find_player(node: &Gd<Node>) -> Option<Gd<AnimationPlayer>> {
    if let Ok(p) = node.clone().try_cast::<AnimationPlayer>() {
        return Some(p);
    }
    for c in node.get_children().iter_shared() {
        if let Some(p) = find_player(&c) {
            return Some(p);
        }
    }
    None
}

/// Runtime glTF scenes keep ImporterMeshInstance3D nodes, which are not
/// drawn. Replace each with a real MeshInstance3D that keeps its skin.
fn realize(node: &Gd<Node>) {
    for c in node.get_children().iter_shared() {
        realize(&c);
    }
    let Ok(imp) = node.clone().try_cast::<ImporterMeshInstance3D>() else {
        return;
    };
    let Some(mesh) = imp.get_mesh().and_then(|m| m.get_mesh()) else {
        return;
    };
    let mut mi = MeshInstance3D::new_alloc();
    mi.set_mesh(&mesh);
    if let Some(skin) = imp.get_skin() {
        mi.set_skin(&skin);
    }
    mi.set_transform(imp.get_transform());
    mi.set_visible(imp.is_visible());
    let name = imp.get_name();
    if let Some(mut parent) = imp.get_parent() {
        let idx = imp.get_index();
        parent.remove_child(&imp);
        mi.set_name(&name);
        parent.add_child(&mi);
        parent.move_child(&mi, idx);
        mi.set_skeleton_path(&imp.get_skeleton_path());
    }
    imp.upcast::<Node>().free();
}

fn meshes(node: &Gd<Node>, out: &mut Vec<Gd<MeshInstance3D>>) {
    if let Ok(m) = node.clone().try_cast::<MeshInstance3D>() {
        out.push(m);
    }
    for c in node.get_children().iter_shared() {
        meshes(&c, out);
    }
}

/// Multiplies every surface's albedo (used to dye cultists, ghouls, bandits).
fn tint(node: &Gd<Node>, color: Color) {
    let mut list = Vec::new();
    meshes(node, &mut list);
    for mut m in list {
        let Some(mesh) = m.get_mesh() else { continue };
        for s in 0..mesh.get_surface_count() {
            if let Some(mat) = m
                .get_active_material(s)
                .and_then(|x| x.try_cast::<StandardMaterial3D>().ok())
            {
                let mut mat = mat.duplicate_resource();
                let base = mat.get_albedo();
                mat.set_albedo(base * color);
                m.set_surface_override_material(s, &mat.upcast::<Material>());
            }
        }
    }
}

/// Adds an emissive glow (elemental enemies).
pub fn glow(model: &ActorModel, color: Color) {
    let Some(rig) = &model.rig_root else { return };
    let mut list = Vec::new();
    meshes(&rig.clone().upcast(), &mut list);
    for mut m in list {
        let Some(mesh) = m.get_mesh() else { continue };
        for s in 0..mesh.get_surface_count() {
            if let Some(mat) = m
                .get_active_material(s)
                .and_then(|x| x.try_cast::<StandardMaterial3D>().ok())
            {
                let mut mat = mat.duplicate_resource();
                mat.set_feature(Feature::EMISSION, true);
                mat.set_emission(color);
                mat.set_emission_energy_multiplier(0.3);
                m.set_surface_override_material(s, &mat.upcast::<Material>());
            }
        }
    }
}

/// Shows only the held items that match the requested weapon and off-hand.
fn arm(scene: &Gd<Node3D>, weapon: &str, offhand: &str) {
    let right: &[&str] = match weapon {
        "axe" => &["Axe"],
        "mace" => &["Mace", "Axe"],
        "staff" => &["Staff"],
        "wand" => &["Wand", "Staff"],
        "dagger" => &["Dagger", "Knife", "1H_"],
        "spear" => &["2H_", "Staff"],
        _ => &["1H_Sword", "Sword", "Blade", "Axe", "Staff"],
    };
    let left: &[&str] = match offhand {
        "shield" => &["Shield"],
        "tome" => &["Spellbook", "Book"],
        _ => &[],
    };
    for (slot, wants) in [("handslot_r", right), ("handslot_l", left)] {
        let Some(hand) = scene
            .find_child_ex(slot)
            .recursive(true)
            .owned(false)
            .done()
            .or_else(|| {
                scene
                    .find_child_ex(&slot.replace('_', "."))
                    .recursive(true)
                    .owned(false)
                    .done()
            })
        else {
            continue;
        };
        let items: Vec<Gd<Node>> = hand.get_children().iter_shared().collect();
        let pick = wants.iter().find_map(|w| {
            items.iter().position(|it| {
                let n = it.get_name().to_string();
                n.contains(w) && !(slot == "handslot_r" && n.contains("Offhand"))
            })
        });
        for (i, it) in items.iter().enumerate() {
            if let Ok(mut n) = it.clone().try_cast::<Node3D>() {
                n.set_visible(Some(i) == pick);
            }
        }
    }
}

/// Builds an animated character from content/models/characters/<file>.glb.
pub fn character(
    lib: &ContentLibrary,
    file: &str,
    weapon: &str,
    offhand: &str,
    dye: Option<Color>,
) -> Option<ActorModel> {
    let mut scene = lib.model_scene(&format!("characters/{file}"))?;
    realize(&scene.clone().upcast());
    scene.set_scale(Vector3::splat(RIG_SCALE));
    arm(&scene, weapon, offhand);
    if let Some(c) = dye {
        tint(&scene.clone().upcast(), c);
    }
    let mut player = find_player(&scene.clone().upcast());
    if let Some(p) = &mut player {
        for name in LOOPING {
            if let Some(mut a) = p.get_animation(name) {
                a.set_loop_mode(LoopMode::LINEAR);
            }
        }
        p.play_ex().name("Idle").done();
    }
    let mut model = ActorModel::empty(1.9);
    model.root.add_child(&scene);
    model.rig = player;
    model.rig_root = Some(scene);
    Some(model)
}

/// Clip for a combat action, with the duration the server expects it to take.
pub fn clip_for(anim: &str) -> Option<(&'static [&'static str], f32)> {
    let d = super::anim::duration(anim);
    let names: &'static [&'static str] = match anim {
        "light1" => &["1H_Melee_Attack_Slice_Horizontal"],
        "light2" => &["1H_Melee_Attack_Slice_Diagonal"],
        "light3" => &["1H_Melee_Attack_Chop"],
        "heavy1" => &["2H_Melee_Attack_Chop"],
        "heavy2" => &["2H_Melee_Attack_Spin", "2H_Melee_Attack_Spinning"],
        "finisher" => &["1H_Melee_Attack_Jump_Chop", "2H_Melee_Attack_Slice"],
        "dodge" => &["Dodge_Forward"],
        "jump" => &["Jump_Full_Short"],
        "block" => &["Blocking"],
        "parry" => &["Block_Attack"],
        "guardbreak" | "stagger" => &["Hit_B"],
        "windup" | "attack" => &["1H_Melee_Attack_Chop", "Unarmed_Melee_Attack_Punch_A"],
        _ => return None,
    };
    Some((names, if d == f32::MAX { 0.0 } else { d }))
}

/// Picks and plays the right clip for this frame. `current` remembers what is
/// playing so clips are not restarted every frame.
pub fn drive(
    player: &mut Gd<AnimationPlayer>,
    current: &mut String,
    anim: &str,
    anim_t: f32,
    speed: f32,
) {
    let active = super::anim::duration(anim) > anim_t && !anim.is_empty() && anim != "idle";
    let (name, rate) = match clip_for(anim).filter(|_| active) {
        Some((names, dur)) => {
            let Some(name) = names.iter().find(|n| player.has_animation(**n)) else {
                return;
            };
            let len = player
                .get_animation(*name)
                .map(|a| a.get_length())
                .unwrap_or(1.0);
            // Enemy wind-ups play the swing slowly so the blow lands when the server resolves it.
            let rate = match anim {
                "windup" => len * 0.45 / 0.6,
                "attack" => 1.6,
                _ if dur > 0.0 => (len / dur).clamp(0.6, 3.0),
                _ => 1.0,
            };
            (name.to_string(), rate)
        }
        None if speed > 3.8 => ("Running_A".to_string(), (speed / 5.5).clamp(0.8, 1.6)),
        None if speed > 0.3 => ("Walking_A".to_string(), (speed / 2.2).clamp(0.7, 1.6)),
        None => ("Idle".to_string(), 1.0),
    };
    let key = format!("{name}#{anim}");
    if *current != key {
        // "attack" continues the wind-up's clip instead of restarting it.
        let restart = !(anim == "attack" && current.starts_with(&name));
        if restart {
            player.play_ex().name(&name).custom_blend(0.12).done();
        }
        player.set_speed_scale(rate);
        *current = key;
    }
}
