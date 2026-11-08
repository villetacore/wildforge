//! Wildforge 3D client as a Godot GDExtension written in Rust.
//!
//! Layers (see docs/ARCHITECTURE.md):
//! - `net`     WebSocket connection + wire protocol
//! - `session` plain data of the current game session
//! - `content` textures/materials/icons from the shared content/ folder
//! - `world`   3D scene: terrain, resource nodes, actors, effects, camera
//! - `ui`      login screen, HUD, crafting window
//! - `input`   keyboard -> commands
//! - `app`     the `WildforgeGame` root node that wires everything together

mod app;
mod content;
mod input;
mod net;
mod session;
mod ui;
mod world;

use godot::prelude::*;

struct WildforgeExtension;

#[gdextension]
unsafe impl ExtensionLibrary for WildforgeExtension {}
