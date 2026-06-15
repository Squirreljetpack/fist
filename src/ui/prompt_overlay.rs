use crate::ui::input::InputWidget;
use crate::{run::action::FsAction, ui::input::InputWidgetConfig};
use matchmaker::{
    action::Action,
    config::{BorderSetting, OverlayLayoutSettings, Percentage},
    ui::{Overlay, OverlayEffect},
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
    pub viewport: Rect,
}

impl PromptOverlay {
    pub fn new(config: PromptConfig) -> Self {
        let input_config = InputWidgetConfig {
            border: config.border.clone(),
            ..Default::default()
        };
        Self {
            input: InputWidget::new(input_config),
            viewport: Rect::default(),
        }
    }

    /// Computes an adaptable area based on viewport size and content width.
    pub fn auto_area(
        &self,
        ui_area: &Rect,
    ) -> Rect {
        let ui_w = ui_area.width;
        let content_w =
            self.input.inner.input.width() as u16 + self.input.config.border.width() + 2;

        // 1. Calculate interpolation factor 't' based on screen width [40, 150]
        let t = ((ui_w as f32 - 40.0) / (150.0 - 40.0)).clamp(0.0, 1.0);

        // 2. Adaptive Minimum: 90% on small screens, 30% on large screens
        let min_p = 0.9 - (t * 0.6);
        let min_limit = (ui_w as f32 * min_p) as u16;

        // 3. Adaptive Maximum: 95% on small screens, capped at 70 chars on large screens
        let max_p = 0.95 - (t * 0.45);
        let max_limit = ((ui_w as f32 * max_p) as u16)
            .max(min_limit)
            .min(70)
            .max(min_limit);

        // 4. Final width: content-driven, clamped by adaptive limits, and screen-safe
        let width = content_w
            .clamp(min_limit, max_limit)
            .min(ui_w.saturating_sub(2));

        // 5. Vertical positioning: 20% offset above center
        let height = self.input.config.border.height() + 1;
        let available_height = ui_area.height.saturating_sub(height);
        let offset =
            available_height / 2 - Percentage::new(20).compute_clamped(available_height, 0, 0);

        Rect {
            x: ui_area.x + (ui_w.saturating_sub(width)) / 2,
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
        _layout: &OverlayLayoutSettings,
    ) {
        self.viewport = *ui_area;
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
    ) {
        let area = self.auto_area(&self.viewport);
        frame.render_widget(Clear, area);

        self.input.update_width(area.width);
        self.input.scroll_to_cursor();

        let block = self.input.config.border.as_block();
        let span = self.input.make_input(ratatui::style::Style::default());
        frame.render_widget(Paragraph::new(Line::from(span)).block(block), area);

        let pos = Position::new(
            area.x + self.input.config.border.left() + self.input.inner.cursor_rel_offset(),
            area.y + self.input.config.border.top(),
        );
        frame.set_cursor_position(pos);
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        if let Some(_accept) = self.input.handle_action(action) {
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
