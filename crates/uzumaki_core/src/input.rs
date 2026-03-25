use std::time::Instant;
use winit::keyboard::{Key, NamedKey};

pub use crate::text_model::{EditEvent, EditKind, KeyResult, Selection, TextModel};

/// Thin widget-layer wrapper around `TextModel`.
/// Holds focus, scroll, blink, disabled, secure, placeholder — presentation concerns.
/// The `TextModel` is the single source of truth for text and selection.
pub struct InputState {
    pub model: TextModel,
    pub placeholder: String,
    pub scroll_offset: f32,
    pub scroll_offset_y: f32,
    pub focused: bool,
    pub blink_reset: Instant,
    pub disabled: bool,
    pub secure: bool,
    pub multiline: bool,
    /// Preserved X coordinate for vertical navigation (sticky column).
    pub sticky_x: Option<f32>,
    /// Preserved column for vertical navigation (sticky column in grapheme units).
    pub sticky_col: Option<usize>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            model: TextModel::new(),
            placeholder: String::new(),
            scroll_offset: 0.0,
            scroll_offset_y: 0.0,
            focused: false,
            blink_reset: Instant::now(),
            disabled: false,
            secure: false,
            multiline: false,
            sticky_x: None,
            sticky_col: None,
        }
    }

    // ── Delegation to model (with disabled guard) ────────────────────

    pub fn insert_text(&mut self, ch: &str) -> Option<EditEvent> {
        self.sticky_x = None;
        self.sticky_col = None;
        if self.disabled {
            return None;
        }
        // Single-line: reject newlines at the boundary
        let text_to_insert;
        let input = if !self.multiline {
            text_to_insert = ch.chars().filter(|&c| c != '\n' && c != '\r').collect::<String>();
            if text_to_insert.is_empty() {
                return None;
            }
            text_to_insert.as_str()
        } else {
            ch
        };
        let result = self.model.insert(input);
        if result.is_some() {
            self.reset_blink();
        }
        result
    }

    pub fn delete_backward(&mut self) -> Option<EditEvent> {
        self.sticky_x = None;
        self.sticky_col = None;
        if self.disabled {
            return None;
        }
        let result = self.model.delete_backward();
        if result.is_some() {
            self.reset_blink();
        }
        result
    }

    pub fn delete_forward(&mut self) -> Option<EditEvent> {
        self.sticky_x = None;
        self.sticky_col = None;
        if self.disabled {
            return None;
        }
        let result = self.model.delete_forward();
        if result.is_some() {
            self.reset_blink();
        }
        result
    }

    pub fn delete_word_backward(&mut self) -> Option<EditEvent> {
        self.sticky_x = None;
        self.sticky_col = None;
        if self.disabled {
            return None;
        }
        let result = self.model.delete_word_backward();
        if result.is_some() {
            self.reset_blink();
        }
        result
    }

    pub fn delete_word_forward(&mut self) -> Option<EditEvent> {
        self.sticky_x = None;
        self.sticky_col = None;
        if self.disabled {
            return None;
        }
        let result = self.model.delete_word_forward();
        if result.is_some() {
            self.reset_blink();
        }
        result
    }

    pub fn move_left(&mut self, extend: bool) {
        self.sticky_x = None;
        self.model.move_left(extend);
        self.reset_blink();
    }

    pub fn move_right(&mut self, extend: bool) {
        self.sticky_x = None;
        self.model.move_right(extend);
        self.reset_blink();
    }

    pub fn move_word_left(&mut self, extend: bool) {
        self.sticky_x = None;
        self.model.move_word_left(extend);
        self.reset_blink();
    }

    pub fn move_word_right(&mut self, extend: bool) {
        self.sticky_x = None;
        self.model.move_word_right(extend);
        self.reset_blink();
    }

    pub fn move_home(&mut self, extend: bool) {
        self.sticky_x = None;
        self.model.move_home(extend);
        self.reset_blink();
    }

    pub fn move_end(&mut self, extend: bool) {
        self.sticky_x = None;
        self.model.move_end(extend);
        self.reset_blink();
    }

    pub fn move_absolute_home(&mut self, extend: bool) {
        self.sticky_x = None;
        self.model.move_absolute_home(extend);
        self.reset_blink();
    }

    pub fn move_absolute_end(&mut self, extend: bool) {
        self.sticky_x = None;
        self.model.move_absolute_end(extend);
        self.reset_blink();
    }

    pub fn move_to(&mut self, pos: usize, extend: bool) {
        self.model.move_to(pos, extend);
        self.reset_blink();
    }

    pub fn select_all(&mut self) {
        self.sticky_x = None;
        self.model.select_all();
        self.reset_blink();
    }

    pub fn word_at(&self, grapheme_idx: usize) -> (usize, usize) {
        self.model.word_at(grapheme_idx)
    }

    pub fn set_value(&mut self, value: String) {
        self.model.set_value(value);
    }

    // ── Widget-layer concerns ────────────────────────────────────────

    pub fn grapheme_count(&self) -> usize {
        self.model.grapheme_count()
    }

    pub fn reset_blink(&mut self) {
        self.blink_reset = Instant::now();
    }

    pub fn blink_visible(&self, window_focused: bool) -> bool {
        if !self.focused || !window_focused {
            return false;
        }
        let elapsed = self.blink_reset.elapsed().as_millis();
        (elapsed % 1060) < 530
    }

    pub fn display_text(&self) -> String {
        if self.secure {
            "\u{2022}".repeat(self.model.grapheme_count())
        } else {
            self.model.text()
        }
    }

    pub fn update_scroll(&mut self, cursor_x: f32, visible_width: f32) {
        if visible_width <= 0.0 {
            return;
        }
        if cursor_x - self.scroll_offset < 0.0 {
            self.scroll_offset = cursor_x;
        } else if cursor_x - self.scroll_offset > visible_width {
            self.scroll_offset = cursor_x - visible_width;
        }
        if self.scroll_offset < 0.0 {
            self.scroll_offset = 0.0;
        }
    }

    pub fn update_scroll_y(&mut self, cursor_y: f32, line_height: f32, visible_height: f32) {
        if visible_height <= 0.0 {
            return;
        }
        let cursor_bottom = cursor_y + line_height;
        if cursor_y < self.scroll_offset_y {
            self.scroll_offset_y = cursor_y;
        } else if cursor_bottom > self.scroll_offset_y + visible_height {
            self.scroll_offset_y = cursor_bottom - visible_height;
        }
        if self.scroll_offset_y < 0.0 {
            self.scroll_offset_y = 0.0;
        }
    }

    /// Handle a key press. Wraps TextModel::handle_key with disabled guard and blink reset.
    /// Returns `Ignored` for ArrowUp/ArrowDown — caller resolves vertical nav via `move_to`.
    pub fn handle_key(&mut self, key: &Key, modifiers: u32) -> KeyResult {
        if self.disabled {
            return KeyResult::Ignored;
        }
        // Single-line: reject Enter
        if !self.multiline {
            if matches!(key, Key::Named(NamedKey::Enter)) {
                return KeyResult::Ignored;
            }
        }
        self.sticky_x = None;
        self.sticky_col = None;
        let result = self.model.handle_key(key, modifiers);
        match &result {
            KeyResult::Edit(_) | KeyResult::Handled | KeyResult::Blur => {
                self.reset_blink();
            }
            KeyResult::Ignored => {}
        }
        result
    }
}
