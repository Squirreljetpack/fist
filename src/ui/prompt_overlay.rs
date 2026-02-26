use std::cell::RefCell;

use crate::run::action::FsAction;
use cli_boilerplate_automation::{auto_impl, define_transparent_wrapper};
use matchmaker::{
    action::Action,
    config::{BorderSetting, InputConfig},
    ui::{Overlay, OverlayEffect, SizeHint},
};
use ratatui::{
    layout::{Position, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use matchmaker::ui::InputUI;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PromptConfig {
    pub border: BorderSetting,
    // pub prompt: String,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            border: BorderSetting {
                sides: Some(Borders::ALL),
                ..Default::default()
            },
            // prompt: String::new(),
        }
    }
}

#[derive(Debug)]
pub struct PromptOverlay(InputUI, Rect);

auto_impl!(PromptOverlay: Deref => InputUI; DerefMut);

impl PromptOverlay {
    pub fn new(config: PromptConfig) -> Self {
        let PromptConfig { border } = config;
        let config = InputConfig {
            border,
            prompt: String::new(),
            ..Default::default()
        };
        let inner = InputUI::new(config);
        Self(inner, Rect::default())
    }
}

impl PromptOverlay {
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
            Action::QueryPos(delta) => {
                todo!()
            }
            Action::Accept => return Some(true),
            Action::Quit(1) => {
                return Some(false);
            }
            _ => {}
        }

        None
    }

    pub fn auto_area(
        &self,
        ui_area: &Rect,
    ) -> Rect {
        let height = self.config.border.height() + 1;
        let width = (ui_area.width * 8 / 10)
            .max(self.input.width() as u16)
            .clamp(12, 70);
        let y = ui_area.y + (ui_area.height.saturating_sub(height + 16)) / 2;

        if width < ui_area.width {
            Rect {
                x: (ui_area.width - width) / 2,
                y,
                width,
                height,
            }
        } else {
            // left align, not center
            Rect {
                x: 1,
                y,
                width: width.min(ui_area.width - 2),
                height,
            }
        }
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
        self.push_char(c);
        OverlayEffect::None
    }

    fn area(
        &mut self,
        ui_area: &Rect,
    ) -> Result<Rect, [SizeHint; 2]> {
        self.1 = self.auto_area(ui_area);
        self.0.update_width(self.1.width);
        Ok(Rect::default())
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
        _area: Rect,
    ) {
        let area = self.1;
        frame.render_widget(Clear, area);
        self.scroll_to_cursor();
        let para = self.make_input();
        frame.render_widget(para, area);

        let pos = self.cursor_offset(&area);
        frame.set_cursor_position(pos);
    }
}
