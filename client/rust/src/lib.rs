//! Wildforge client as a Godot GDExtension written in Rust.
//! The whole client (UI, networking, rendering) lives in the `WildforgeGame` node.

mod game;
mod net;
mod ui;

use godot::prelude::*;

struct WildforgeExtension;

#[gdextension]
unsafe impl ExtensionLibrary for WildforgeExtension {}
