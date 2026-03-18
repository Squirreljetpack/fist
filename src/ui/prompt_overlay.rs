use crate::ui::input::InputWidget;
use crate::{run::action::FsAction, ui::input::InputWidgetConfig};
use matchmaker::{
    action::Action,
    config::{BorderSetting, Percentage},
    ui::{Overlay, OverlayEffect, SizeHint},
};
use ratatui::{
    layout::{Position, Rect},
    text::Line,
    widgets::{Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PromptConfig {
    pub border: BorderSetting,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            border: BorderSetting {
                sides: Some(Borders::ALL),
                ..Default::default()
            },
        }
    }
}

pub struct PromptOverlay {
    pub input: InputWidget,
    pub area: Rect,
    pub config: PromptConfig,
}

impl PromptOverlay {
    pub fn new(config: PromptConfig) -> Self {
        let input_config = InputWidgetConfig {
            border: config.border.clone(),
            ..Default::default()
        };
        Self {
            input: InputWidget::new(input_config),
            area: Rect::default(),
            config,
        }
    }

    pub fn auto_area(
        &self,
        ui_area: &Rect,
    ) -> Rect {
        let height = self.config.border.height() + 1;
        let width = if (ui_area.width * 7 / 10) < self.input.inner.input.width() as u16 {
            (self.input.inner.input.width() as u16)
                .min(ui_area.width.saturating_sub(self.config.border.width() + 2))
        } else {
            (ui_area.width * 7 / 10).min(70)
        };

        let available_height = ui_area.height.saturating_sub(height);
        let offset =
            available_height / 2 - Percentage::new(20).compute_clamped(available_height, 0, 0);

        Rect {
            x: (ui_area.width - width) / 2,
            y: ui_area.y + offset,
            width,
            height,
        }
    }
}

impl Overlay for PromptOverlay {
    type A = FsAction;

    fn on_disable(&mut self) {
        self.input.inner.cancel();
    }

    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        if c == '\n' {
            return OverlayEffect::Disable;
        }
        self.input.handle_input(c);
        OverlayEffect::None
    }

    fn area(
        &mut self,
        ui_area: &Rect,
    ) -> Result<Rect, [SizeHint; 2]> {
        self.area = self.auto_area(ui_area);
        Ok(Rect::default())
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
        _area: Rect,
    ) {
        frame.render_widget(Clear, self.area);

        let input_width = self.area.width.saturating_sub(self.config.border.width());
        let span = self.input.make_input(input_width, ratatui::style::Style::default());

        frame.render_widget(
            Paragraph::new(Line::from(span)).block(self.config.border.as_block()),
            self.area,
        );

        let pos = Position::new(
            self.area.x + self.config.border.left() + self.input.inner.cursor_rel_offset(),
            self.area.y + self.config.border.top(),
        );
        frame.set_cursor_position(pos);
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        if let Some(accepted) = self.input.handle_action(action) {
            return OverlayEffect::Disable;
        }
        OverlayEffect::None
    }
}

impl PromptOverlay {
    pub fn handle_action_(
        &mut self,
        action: &Action<FsAction>,
    ) -> Option<bool> {
        self.input.handle_action(action)
    }
}
