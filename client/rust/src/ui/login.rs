//! Login / registration screen.

use godot::classes::text_server::AutowrapMode;
use godot::classes::{CanvasLayer, ColorRect, Label, LineEdit, VBoxContainer};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

use super::{button, label, line_edit, panel, push, UiEvent, UiQueue, SCREEN_H, SCREEN_W};

pub struct LoginUi {
    pub layer: Gd<CanvasLayer>,
    pub server: Gd<LineEdit>,
    pub username: Gd<LineEdit>,
    pub password: Gd<LineEdit>,
    status: Gd<Label>,
}

impl LoginUi {
    pub fn build(q: &UiQueue) -> LoginUi {
        let mut layer = CanvasLayer::new_alloc();
        let mut bg = ColorRect::new_alloc();
        bg.set_color(Color::from_rgb(0.07, 0.1, 0.09));
        bg.set_size(Vector2::new(SCREEN_W, SCREEN_H));
        layer.add_child(&bg);

        let mut p = panel(
            Vector2::new(SCREEN_W / 2.0 - 200.0, 130.0),
            Vector2::new(400.0, 0.0),
            0.95,
        );
        let mut v = VBoxContainer::new_alloc();
        v.add_theme_constant_override("separation", 8);

        let mut title = label("WILDFORGE", 40);
        title.set_horizontal_alignment(HorizontalAlignment::CENTER);
        v.add_child(&title);
        let mut sub = label("gather · craft · fight · together", 14);
        sub.set_horizontal_alignment(HorizontalAlignment::CENTER);
        sub.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.6));
        v.add_child(&sub);

        v.add_child(&label("Server", 13));
        let server = line_edit("ws://host:7777/ws", "ws://127.0.0.1:7777/ws");
        v.add_child(&server);
        v.add_child(&label("Username", 13));
        let username = line_edit("3-16 letters, digits or _", "");
        v.add_child(&username);
        v.add_child(&label("Password", 13));
        let mut password = line_edit("at least 4 characters", "");
        password.set_secret(true);
        v.add_child(&password);

        let login = button("Log in");
        let register = button("Create account");
        v.add_child(&login);
        v.add_child(&register);

        let mut status = label("", 14);
        status.set_autowrap_mode(AutowrapMode::WORD);
        status.set_custom_minimum_size(Vector2::new(0.0, 40.0));
        v.add_child(&status);

        p.add_child(&v);
        layer.add_child(&p);

        let qc = q.clone();
        login
            .signals()
            .pressed()
            .connect(move || push(&qc, UiEvent::Login));
        let qc = q.clone();
        register
            .signals()
            .pressed()
            .connect(move || push(&qc, UiEvent::Register));
        let qc = q.clone();
        password
            .signals()
            .text_submitted()
            .connect(move |_t: GString| push(&qc, UiEvent::Login));

        LoginUi {
            layer,
            server,
            username,
            password,
            status,
        }
    }

    pub fn set_status(&mut self, text: &str, error: bool) {
        self.status.set_text(text);
        let c = if error {
            Color::from_rgb(1.0, 0.45, 0.4)
        } else {
            Color::from_rgb(0.6, 1.0, 0.6)
        };
        self.status.add_theme_color_override("font_color", c);
    }

    /// (server url, username, password) with the url normalized to ws://host:port/ws.
    pub fn credentials(&self) -> (String, String, String) {
        let mut url = self.server.get_text().to_string().trim().to_string();
        if !url.contains("://") {
            url = format!("ws://{url}");
        }
        if url.matches('/').count() < 3 {
            url.push_str("/ws");
        }
        (
            url,
            self.username.get_text().to_string().trim().to_string(),
            self.password.get_text().to_string(),
        )
    }
}
