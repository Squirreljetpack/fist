use crate::run::action::FsAction;
use matchmaker::{
    action::Action,
    config::{BorderSetting, InputConfig},
    ui::InputUI,
};
use ratatui::layout::Rect;

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputWidgetConfig {
    pub border: BorderSetting,
}

pub struct InputWidget {
    pub inner: InputUI,
}

impl InputWidget {
    pub fn new(config: InputWidgetConfig) -> Self {
        let inner_config = InputConfig {
            border: config.border,
            prompt: String::new(),
            ..Default::default()
        };
        Self {
            inner: InputUI::new(inner_config),
        }
    }

    pub fn set_value(
        &mut self,
        value: String,
    ) {
        self.inner.set(value, u16::MAX);
    }

    pub fn value(&self) -> String {
        self.inner.input.clone()
    }

    pub fn handle_input(
        &mut self,
        c: char,
    ) {
        self.inner.push_char(c);
    }

    pub fn handle_action(
        &mut self,
        action: &Action<FsAction>,
    ) -> Option<bool> {
        match action {
            Action::ForwardChar => self.inner.forward_char(),
            Action::BackwardChar => self.inner.backward_char(),
            Action::ForwardWord => self.inner.forward_word(),
            Action::BackwardWord => self.inner.backward_word(),
            Action::DeleteChar => self.inner.delete(),
            Action::DeleteWord => self.inner.delete_word(),
            Action::DeleteLineStart => self.inner.delete_line_start(),
            Action::DeleteLineEnd => self.inner.delete_line_end(),
            Action::Cancel | Action::Quit(1) => {
                self.inner.cancel();
                return Some(false);
            }
            Action::Accept => return Some(true),
            _ => {}
        }
        None
    }

    pub fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
        area: Rect,
    ) {
        self.inner.update_width(area.width);
        self.inner.scroll_to_cursor();
        let para = self.inner.make_input();
        frame.render_widget(para, area);

        let pos = self.inner.cursor_offset(&area);
        frame.set_cursor_position(pos);
    }
}
