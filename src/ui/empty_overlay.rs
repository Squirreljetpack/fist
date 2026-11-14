use matchmaker::{
    action::Action,
    config::{FrameSetting, OverlayLayoutSettings},
    render::MMState,
    ui::{Frame, Overlay, OverlayEffect, Rect, SizeHint, utils},
};
use ratatui::{
    prelude::*,
    widgets::{Clear, Paragraph},
};

use crate::run::{action::FsAction, item::PathItem};

/// Static message overlay shown when opening the menu in panes with no actions
/// (such as the app pane).
pub struct EmptyOverlay {
    frame: FrameSetting,
    min_width: u16,
    message: &'static str,
    area: Rect,
}

impl EmptyOverlay {
    pub fn new(
        frame: FrameSetting,
        min_width: u16,
        message: &'static str,
    ) -> Self {
        Self {
            frame,
            min_width,
            message,
            area: Rect::default(),
        }
    }
}

impl Overlay<FsAction, PathItem, ()> for EmptyOverlay {
    fn handle_input(
        &mut self,
        _c: char,
        _state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        OverlayEffect::None
    }

    fn handle_action(
        &mut self,
        action: &Action<FsAction>,
        _state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        match action {
            Action::Quit(_) | Action::Custom(FsAction::ShowMenu) => OverlayEffect::Disable,
            _ => OverlayEffect::None,
        }
    }

    fn area(
        &mut self,
        ui_area: &Rect,
        layout: &OverlayLayoutSettings,
    ) {
        let msg_width = Text::from(self.message).width() as u16;
        let msg_height = self.message.lines().count().max(1) as u16;
        let width = self.min_width.max(msg_width) + self.frame.width();
        let height = msg_height + self.frame.height();
        self.area = utils::default_area(
            [SizeHint::from(width), SizeHint::from(height)],
            layout,
            ui_area,
        );
    }

    fn draw(
        &mut self,
        frame: &mut Frame,
    ) {
        frame.render_widget(Clear, self.area);
        frame.render_widget(self.frame.as_block(), self.area);
        let inner = self.frame.inner_of(self.area);
        frame.render_widget(
            Paragraph::new(Text::from(self.message)).alignment(Alignment::Center),
            inner,
        );
    }
}
