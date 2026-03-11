use crate::run::action::FsAction;
use crate::run::state::TlsStore;
use crate::utils::serde::border_result;
use cba::bait::TransformExt;
use matchmaker::{
    action::Action,
    config::{BorderSetting, PartialBorderSetting},
    ui::{Overlay, OverlayEffect, SizeHint},
};
use ratatui::{
    prelude::*,
    widgets::{BorderType, Borders, Clear, Padding, Paragraph},
};
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfirmConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    pub content_border: BorderSetting,
    pub button_fg: Color,
    pub button_bg: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
    pub hotkey_modifier: Modifier,
    pub options_alignment: HorizontalAlignment,
    pub options_gap: u16,
}

impl Default for ConfirmConfig {
    fn default() -> Self {
        Self {
            border: Err(PartialBorderSetting {
                sides: Some(Borders::ALL),
                color: Some(Color::White),
                padding: Some(Padding::new(2, 2, 0, 1).into()),
                ..Default::default()
            }),
            content_border: BorderSetting {
                r#type: Some(BorderType::LightDoubleDashed),
                ..Default::default()
            },
            button_fg: Color::Reset,
            button_bg: Color::Reset,
            selected_fg: Color::Black,
            selected_bg: Color::White,
            hotkey_modifier: Modifier::BOLD,
            options_alignment: HorizontalAlignment::Center,
            options_gap: 2,
        }
    }
}

pub struct ConfirmPrompt {
    pub prompt: Line<'static>,
    pub options: Vec<(&'static str, usize)>,
    pub option_handler: Box<dyn FnOnce(usize)>,
    pub content: Option<Text<'static>>,
    pub content_above: bool,
    pub title_in_border: bool,
    pub cursor: usize,
    pub scroll: u16,
}

impl ConfirmPrompt {
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(
        &mut self,
        cursor: usize,
    ) {
        self.cursor = cursor;
    }
}

impl Default for ConfirmPrompt {
    fn default() -> Self {
        Self {
            prompt: Line::from("Confirm?"),
            options: vec![("Yes", 0), ("No", 0)],
            option_handler: Box::new(|idx| {
                TlsStore::set(ConfirmResult(idx == 0));
            }),
            content: None,
            content_above: false,
            title_in_border: true,
            cursor: 1,
            scroll: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfirmResult(pub bool);

pub struct ConfirmOverlay {
    pub config: ConfirmConfig,
    pub prompt: ConfirmPrompt,
}

impl ConfirmOverlay {
    pub fn new(config: ConfirmConfig) -> Self {
        Self {
            config,
            prompt: ConfirmPrompt::default(),
        }
    }

    fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }

    fn content_border(&self) -> &BorderSetting {
        &self.config.content_border
    }

    fn accept(&mut self) -> OverlayEffect {
        let handler = std::mem::replace(
            &mut self.prompt.option_handler,
            ConfirmPrompt::default().option_handler,
        );
        handler(self.prompt.cursor());
        OverlayEffect::Disable
    }
}

impl Overlay for ConfirmOverlay {
    type A = FsAction;

    fn on_enable(
        &mut self,
        _area: &Rect,
    ) {
        if let Some(prompt) = TlsStore::take::<ConfirmPrompt>() {
            self.prompt = prompt;
        } else {
            self.prompt = ConfirmPrompt::default();
        }
    }

    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        for (i, (name, trigger_idx)) in self.prompt.options.iter().enumerate() {
            if let Some(trigger_char) = name.chars().nth(*trigger_idx) {
                if trigger_char.eq_ignore_ascii_case(&c) {
                    self.prompt.set_cursor(i);
                    return self.accept();
                }
            }
        }
        OverlayEffect::None
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        match action {
            Action::ForwardChar => {
                let next = (self.prompt.cursor() + 1) % self.prompt.options.len();
                self.prompt.set_cursor(next);
            }
            Action::BackwardChar => {
                let cur = self.prompt.cursor();
                let next = if cur == 0 {
                    self.prompt.options.len() - 1
                } else {
                    cur - 1
                };
                self.prompt.set_cursor(next);
            }
            Action::Up(n) => {
                self.prompt.scroll = self.prompt.scroll.saturating_sub(*n);
            }
            Action::Down(n) => {
                self.prompt.scroll = self.prompt.scroll.saturating_add(*n);
            }
            Action::HalfPageUp => {
                self.prompt.scroll = self.prompt.scroll.saturating_sub(5);
            }
            Action::HalfPageDown => {
                self.prompt.scroll = self.prompt.scroll.saturating_add(5);
            }
            Action::Accept => return self.accept(),
            Action::Quit(_) => return OverlayEffect::Disable,
            _ => {}
        }
        OverlayEffect::None
    }

    fn area(
        &mut self,
        ui_area: &Rect,
    ) -> Result<Rect, [SizeHint; 2]> {
        let has_title = !self.prompt.title_in_border;
        let has_content = self.prompt.content.is_some();
        let title_h = has_title as u16;
        let min_content_h = if has_content {
            5 + self.content_border().height()
        } else {
            0
        };

        let num_gaps = if has_title && has_content {
            2
        } else if has_title || has_content {
            1
        } else {
            0
        };

        let options_width = self
            .prompt
            .options
            .iter()
            .map(|(s, _)| s.len() as u16 + self.config.options_gap)
            .sum::<u16>();

        let width = 20
            .max(self.prompt.prompt.width() as u16)
            .max(options_width)
            .max(
                self.prompt
                    .content
                    .as_ref()
                    .map(|t| t.width() as u16)
                    .unwrap_or_default(),
            )
            .transform(|x| x + self.border().width())
            .min(ui_area.width.saturating_sub(2) * 9 / 10);

        let options_h = options_width.div_ceil(ui_area.width);

        let height = if has_content {
            SizeHint::Min(title_h + options_h + min_content_h + num_gaps + self.border().height())
        } else {
            (title_h + options_h + num_gaps + self.border().height()).into()
        };

        Err([width.into(), height])
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame,
        area: Rect,
    ) {
        // 1. Initial Setup & Clearing
        frame.render_widget(Clear, area);
        let block = if !self.prompt.title_in_border {
            self.border().as_block()
        } else {
            self.border().as_block().title(self.prompt.prompt.clone())
        };
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let has_title = !self.prompt.title_in_border;
        let has_content = self.prompt.content.is_some();
        let title_h = has_title as u16;
        let spacing = 1; // Vertical gap between components
        let num_gaps = if has_title && has_content {
            2
        } else if has_title || has_content {
            1
        } else {
            0
        };

        // 2. Wrap Options into Rows
        // We need to know how many rows exist to calculate the layout heights
        let mut rows: Vec<Vec<(usize, &&str, &usize, u16)>> = vec![vec![]];
        let mut current_row_width = 0;
        let default_gap = 2;

        for (i, (name, trigger_idx)) in self.prompt.options.iter().enumerate() {
            let btn_width = name.len() as u16 + self.config.options_gap;

            // If this isn't the first item in the row, check if we need to wrap
            if !rows.last().unwrap().is_empty()
                && current_row_width + default_gap + btn_width > inner_area.width
            {
                rows.push(vec![]);
                current_row_width = 0;
            }

            let gap = if rows.last().unwrap().is_empty() {
                0
            } else {
                default_gap
            };
            current_row_width += gap + btn_width;
            rows.last_mut()
                .unwrap()
                .push((i, name, trigger_idx, btn_width));
        }

        let options_h = rows.len() as u16;

        // 3. Calculate Content Height
        let content_h = inner_area
            .height
            .saturating_sub(title_h)
            .saturating_sub(options_h)
            .saturating_sub(num_gaps * spacing);

        let mut current_y = inner_area.y;

        // 4. Render Title
        if has_title {
            let title_area = Rect::new(inner_area.x, current_y, inner_area.width, 1);
            frame.render_widget(Paragraph::new(self.prompt.prompt.clone()), title_area);
            current_y += 1 + spacing;
        }

        // 5. Determine Position for Options and Content
        let options_area_y = if self.prompt.content_above {
            inner_area.bottom().saturating_sub(options_h)
        } else {
            current_y
        };

        // 6. Build and Render Options Paragraph
        let mut option_lines: Vec<Line> = Vec::new();

        for row in rows {
            let mut line_spans = Vec::new();
            let num_buttons = row.len();

            // Compute inner spacing based on alignment
            let (gap_width, leading_gap) = if self.config.options_alignment == Alignment::Center {
                let total_btns_width: u16 = row.iter().map(|(.., w)| *w).sum();
                let gap =
                    (inner_area.width.saturating_sub(total_btns_width)) / (num_buttons as u16 + 1);
                (gap, gap)
            } else {
                (default_gap, 0)
            };

            if leading_gap > 0 {
                line_spans.push(Span::raw(" ".repeat(leading_gap as usize)));
            }

            for (i, (idx, name, trigger_idx, _)) in row.into_iter().enumerate() {
                let is_last = i == num_buttons - 1;

                let style = if idx == self.prompt.cursor() {
                    Style::default()
                        .bg(self.config.selected_bg)
                        .fg(self.config.selected_fg)
                } else {
                    Style::default()
                        .bg(self.config.button_bg)
                        .fg(self.config.button_fg)
                };

                // Individual character rendering for hotkeys
                for (char_idx, c) in name.chars().enumerate() {
                    let s = if char_idx == *trigger_idx {
                        style.add_modifier(self.config.hotkey_modifier)
                    } else {
                        style
                    };
                    line_spans.push(Span::styled(c.to_string(), s));
                }

                // Spacing logic
                if !is_last || self.config.options_alignment == Alignment::Center {
                    line_spans.push(Span::raw(" ".repeat(gap_width as usize)));
                }
            }
            option_lines.push(Line::from(line_spans));
        }

        let options_area = Rect::new(inner_area.x, options_area_y, inner_area.width, options_h);

        // If Centered, we manually padded with spans, so we use Left alignment to keep our math stable.
        // Otherwise, we use the requested Alignment.
        let final_alignment = if self.config.options_alignment == Alignment::Center {
            Alignment::Left
        } else {
            self.config.options_alignment
        };

        frame.render_widget(
            Paragraph::new(option_lines).alignment(final_alignment),
            options_area,
        );

        // 7. Render Content Block
        if let Some(content) = &self.prompt.content {
            let content_y = if self.prompt.content_above {
                current_y
            } else {
                options_area_y + options_h + spacing
            };

            let max_scroll = (content.height() as u16).saturating_sub(content_h);
            self.prompt.scroll = self.prompt.scroll.min(max_scroll);

            let content_area = Rect::new(inner_area.x, content_y, inner_area.width, content_h);
            frame.render_widget(
                Paragraph::new(content.clone())
                    .block(self.content_border().as_block())
                    .scroll((self.prompt.scroll, 0)),
                content_area,
            );
        }
    }
}

// -------------- BOILERPLATE -----------------
impl Debug for ConfirmPrompt {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("ConfirmPrompt")
            .field("prompt", &self.prompt)
            .field("options", &self.options)
            .field("content", &self.content)
            .field("content_above", &self.content_above)
            .field("title_in_border", &self.title_in_border)
            .field("cursor", &self.cursor)
            .field("scroll", &self.scroll)
            .finish()
    }
}
