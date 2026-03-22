use crate::run::action::FsAction;
use matchmaker::config::StyleSetting;
use matchmaker::{action::Action, config::BorderSetting, ui::InputUI};
use ratatui::style::Style;
use ratatui::text::Span;

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputWidgetConfig {
    pub border: BorderSetting,
    pub no_scroll_padding: bool,
    pub style: StyleSetting,
}

pub struct InputWidget {
    pub inner: InputUI,
    pub config: InputWidgetConfig,
}

impl InputWidget {
    pub fn new(config: InputWidgetConfig) -> Self {
        Self {
            inner: InputUI::new(),
            config,
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

    pub fn make_input(
        &mut self,
        width: u16,
        style: Style,
    ) -> Span<'_> {
        self.update_scroll(width);
        Span::styled(self.inner.render(), style)
    }

    pub fn update_scroll(
        &mut self,
        width: u16,
    ) {
        self.inner.width = width;
        let padding = if self.config.no_scroll_padding { 0 } else { 2 };
        self.inner.scroll_to_cursor(padding);
    }
}
