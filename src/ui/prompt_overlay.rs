use std::cell::RefCell;

use crate::run::{action::FsAction, globals::TEMP};
use crate::utils::text::grapheme_index_to_byte_index;
use matchmaker::{
    action::Action,
    config::BorderSetting,
    ui::{Overlay, OverlayEffect},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PromptConfig {
    pub border: BorderSetting,
    prompt: String,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            border: BorderSetting {
                sides: Borders::ALL,
                ..Default::default()
            },
            prompt: String::new(),
        }
    }
}

pub struct PromptOverlay {
    pub input: String,
    pub cursor: u16, // grapheme index
    pub config: PromptConfig,
    pub ui_area: Rect,
}

impl PromptOverlay {
    pub fn new(config: PromptConfig) -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            ui_area: Rect::default(),
            config,
        }
    }
    // ---------- GETTERS ---------
    pub fn len(&self) -> usize {
        self.input.len()
    }
    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    pub fn cursor_offset(
        &self,
        rect: &Rect,
    ) -> Position {
        let left = self.config.border.left();
        let top = self.config.border.top();
        Position::new(rect.x + self.cursor + left, rect.y + top)
    }
    // ------------ SETTERS ---------------
    pub fn set(
        &mut self,
        input: String,
        cursor: u16,
    ) {
        let grapheme_count = input.graphemes(true).count() as u16;
        self.input = input;
        self.cursor = cursor.min(grapheme_count);
    }
    pub fn cancel(&mut self) {
        self.input.clear();
        self.cursor = 0;
    }

    // ---------- EDITING -------------
    pub fn forward_char(&mut self) {
        // Check against the total number of graphemes
        if self.cursor < self.input.graphemes(true).count() as u16 {
            self.cursor += 1;
        }
    }
    pub fn backward_char(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }
    pub fn insert_char(
        &mut self,
        c: char,
    ) {
        let old_grapheme_count = self.input.graphemes(true).count() as u16;
        let byte_index = grapheme_index_to_byte_index(&self.input, self.cursor);
        self.input.insert(byte_index, c);
        let new_grapheme_count = self.input.graphemes(true).count() as u16;
        if new_grapheme_count > old_grapheme_count {
            self.cursor += 1;
        }
    }

    pub fn forward_word(&mut self) {
        let post = self.input.graphemes(true).skip(self.cursor as usize);

        let mut in_word = false;

        for g in post {
            self.cursor += 1;
            if g.chars().all(|c| c.is_whitespace()) {
                if in_word {
                    return;
                }
            } else {
                in_word = true;
            }
        }
    }

    pub fn backward_word(&mut self) {
        let mut in_word = false;

        let pre: Vec<&str> = self
            .input
            .graphemes(true)
            .take(self.cursor as usize)
            .collect();

        for g in pre.iter().rev() {
            self.cursor -= 1;

            if g.chars().all(|c| c.is_whitespace()) {
                if in_word {
                    return;
                }
            } else {
                in_word = true;
            }
        }

        self.cursor = 0;
    }

    pub fn delete(&mut self) {
        if self.cursor > 0 {
            let byte_start = grapheme_index_to_byte_index(&self.input, self.cursor - 1);
            let byte_end = grapheme_index_to_byte_index(&self.input, self.cursor);

            self.input.replace_range(byte_start..byte_end, "");
            self.cursor -= 1;
        }
    }

    pub fn delete_word(&mut self) {
        let old_cursor_grapheme = self.cursor;
        self.backward_word();
        let new_cursor_grapheme = self.cursor;

        let byte_start = grapheme_index_to_byte_index(&self.input, new_cursor_grapheme);
        let byte_end = grapheme_index_to_byte_index(&self.input, old_cursor_grapheme);

        self.input.replace_range(byte_start..byte_end, "");
    }

    pub fn delete_line_start(&mut self) {
        let byte_end = grapheme_index_to_byte_index(&self.input, self.cursor);

        self.input.replace_range(0..byte_end, "");
        self.cursor = 0;
    }

    pub fn delete_line_end(&mut self) {
        let byte_index = grapheme_index_to_byte_index(&self.input, self.cursor);

        // Truncate operates on the byte index
        self.input.truncate(byte_index);
    }

    // Some(true) -> success
    pub fn handle_action_(
        &mut self,
        action: &Action<FsAction>,
    ) -> Option<bool> {
        match action {
            Action::ForwardChar => self.forward_char(),
            Action::BackwardChar => self.backward_char(),
            Action::ForwardWord => self.forward_word(),
            Action::BackwardWord => self.backward_word(),
            Action::DeleteChar => self.delete(),
            Action::DeleteWord => self.delete_word(),
            Action::DeleteLineStart => self.delete_line_start(),
            Action::DeleteLineEnd => self.delete_line_end(),
            Action::Cancel => self.cancel(),
            Action::InputPos(delta) => {
                let len = self.input.graphemes(true).count() as i32;
                let new = self.cursor as i32 + *delta;
                self.cursor = new.clamp(0, len) as u16;
            }
            Action::Accept => return Some(true),
            Action::Quit(1) => {
                return Some(false);
            }
            _ => {}
        }

        None
    }
}

impl Overlay for PromptOverlay {
    type A = FsAction;

    fn on_disable(&mut self) {
        self.cancel();
    }

    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        if c == '\n' {
            return OverlayEffect::Disable;
        }
        self.insert_char(c);
        OverlayEffect::None
    }

    fn area(
        &mut self,
        ui_area: &Rect,
    ) -> Result<Rect, [u16; 2]> {
        // recompute
        self.ui_area = *ui_area;
        Ok(Rect::default())
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
        _area: Rect,
    ) {
        let para = Paragraph::new(self.input.as_str())
            .left_aligned()
            .block(self.config.border.as_block());

        let height = self.config.border.height() + 1;
        let width = (self.ui_area.width * 8 / 10)
            .max(self.input.width() as u16)
            .clamp(12, 70);
        let y = self.ui_area.y + (self.ui_area.height.saturating_sub(height + 16)) / 2;

        let area = if width < self.ui_area.width {
            Rect {
                x: (self.ui_area.width - width) / 2,
                y,
                width,
                height,
            }
        } else {
            // left align, not center
            Rect {
                x: 1,
                y,
                width: width.min(self.ui_area.width - 2),
                height,
            }
        };

        frame.render_widget(Clear, area);
        frame.render_widget(para, area);

        let input_str = self
            .input
            .graphemes(true)
            .take(self.cursor as usize)
            .collect::<String>();
        let cursor_x = area.x
            + UnicodeWidthStr::width(input_str.as_str()) as u16
            + self.config.border.width() / 2;
        frame.set_cursor_position((cursor_x, area.y + 1));
    }
}
