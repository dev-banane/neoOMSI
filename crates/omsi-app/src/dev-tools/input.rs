#![allow(unused_imports)]
use super::DevTools;
use imgui::{Key, MouseButton};
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

impl DevTools {
    pub(crate) fn event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::KeyboardInput { event: k, .. } = event {
            if k.physical_key == PhysicalKey::Code(KeyCode::Backquote) {
                if k.state == ElementState::Pressed && !k.repeat {
                    self.visible = !self.visible;
                }
                return true;
            }
        }
        if !self.visible {
            return false;
        }
        let io = self.ctx.io_mut();
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                io.add_mouse_pos_event([position.x as f32, position.y as f32]);
                false
            }
            WindowEvent::CursorLeft { .. } => {
                io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return false,
                };
                let was_over = io.want_capture_mouse;
                io.add_mouse_button_event(b, *state == ElementState::Pressed);
                was_over && *state == ElementState::Pressed
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        (p.x as f32 / 40.0, p.y as f32 / 40.0)
                    }
                };
                io.add_mouse_wheel_event([x, y]);
                io.want_capture_mouse
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                io.add_key_event(Key::ModCtrl, s.control_key());
                io.add_key_event(Key::ModShift, s.shift_key());
                io.add_key_event(Key::ModAlt, s.alt_key());
                io.add_key_event(Key::ModSuper, s.super_key());
                false
            }
            WindowEvent::KeyboardInput { event: k, .. } => {
                let down = k.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = k.physical_key {
                    if let Some(key) = map_key(code) {
                        io.add_key_event(key, down);
                    }
                }
                if down {
                    if let Some(text) = k.text.as_deref() {
                        for c in text.chars().filter(|c| !c.is_control()) {
                            io.add_input_character(c);
                        }
                    }
                }
                io.want_capture_keyboard
            }
            _ => false,
        }
    }
}

fn map_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::ArrowLeft => Key::LeftArrow,
        KeyCode::ArrowRight => Key::RightArrow,
        KeyCode::ArrowUp => Key::UpArrow,
        KeyCode::ArrowDown => Key::DownArrow,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyX => Key::X,
        _ => return None,
    })
}
