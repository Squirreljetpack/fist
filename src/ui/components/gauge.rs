use cba::bring::StrExt;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};
use std::fmt::Alignment;

/// A progress gauge component that renders a percentage/text over a progress bar.
///
/// Filled portions render with a solid background and contrasting text color.
/// Unfilled/remaining portions render with dotted characters (or spaces) and
/// contrasting text color so the label remains legible across the split.
#[derive(Debug, Clone)]
pub struct Gauge {
    ratio: f64,
    label: Option<String>,
    filled_style: Style,
    unfilled_style: Style,
    unfilled_text_style: Style,
    unfilled_char: char,
}

impl Default for Gauge {
    fn default() -> Self {
        Self {
            ratio: 0.0,
            label: None,
            filled_style: Style::default().bg(Color::Cyan).fg(Color::Black),
            unfilled_style: Style::default().fg(Color::DarkGray),
            unfilled_text_style: Style::default().fg(Color::Cyan),
            unfilled_char: '·',
        }
    }
}

impl Gauge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ratio(
        mut self,
        ratio: f64,
    ) -> Self {
        self.ratio = ratio.clamp(0.0, 1.0);
        self
    }

    pub fn percent(
        mut self,
        percent: f64,
    ) -> Self {
        self.ratio = (percent / 100.0).clamp(0.0, 1.0);
        self
    }

    pub fn label<S: Into<String>>(
        mut self,
        label: S,
    ) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn filled_style(
        mut self,
        style: Style,
    ) -> Self {
        self.filled_style = style;
        self
    }

    pub fn unfilled_style(
        mut self,
        style: Style,
    ) -> Self {
        self.unfilled_style = style;
        self
    }

    pub fn unfilled_text_style(
        mut self,
        style: Style,
    ) -> Self {
        self.unfilled_text_style = style;
        self
    }

    pub fn unfilled_char(
        mut self,
        c: char,
    ) -> Self {
        self.unfilled_char = c;
        self
    }

    /// Render the gauge as a [`Line`] of width `width`.
    pub fn render_line(
        &self,
        width: usize,
    ) -> Line<'static> {
        if width == 0 {
            return Line::from("");
        }

        let label_text = match &self.label {
            Some(l) => l.clone(),
            None => {
                let percent = self.ratio * 100.0;
                if width <= 4 {
                    format!("{:.0}%", percent)
                } else if width <= 6 {
                    format!("{:>3.0}%", percent)
                } else if width <= 8 {
                    format!("{:5.1}%", percent)
                } else {
                    format!("{:5.2}%", percent)
                }
            }
        };

        let padded_str = label_text.pad_to(width, Alignment::Center);
        let chars: Vec<char> = padded_str.chars().take(width).collect();
        let filled_count = ((self.ratio * width as f64).round() as usize).min(width);

        let mut spans: Vec<Span<'static>> = Vec::new();
        for (idx, &c) in chars.iter().enumerate() {
            let (ch, style) = if idx < filled_count {
                (c, self.filled_style)
            } else if c == ' ' {
                (self.unfilled_char, self.unfilled_style)
            } else {
                (c, self.unfilled_text_style)
            };

            if let Some(last) = spans.last_mut() {
                if last.style == style {
                    let mut s = last.content.to_string();
                    s.push(ch);
                    *last = Span::styled(s, style);
                    continue;
                }
            }
            spans.push(Span::styled(ch.to_string(), style));
        }

        Line::from(spans)
    }
}

impl Widget for Gauge {
    fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
    ) {
        (&self).render(area, buf);
    }
}

impl Widget for &Gauge {
    fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let line = self.render_line(area.width as usize);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauge_render_empty() {
        let gauge = Gauge::new().ratio(0.0);
        let line = gauge.render_line(10);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().count(), 10);
        assert!(text.contains("0.00%"));
        assert!(text.contains('·'));
        // 0% filled: no spans with filled_style
        for span in &line.spans {
            assert_ne!(span.style, gauge.filled_style);
        }
    }

    #[test]
    fn test_gauge_render_full() {
        let gauge = Gauge::new().ratio(1.0);
        let line = gauge.render_line(10);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().count(), 10);
        assert!(text.contains("100.00%"));
        assert!(!text.contains('·'));
        // 100% filled: all characters styled with filled_style
        for span in &line.spans {
            assert_eq!(span.style, gauge.filled_style);
        }
    }

    #[test]
    fn test_gauge_render_half() {
        let gauge = Gauge::new().ratio(0.5);
        let line = gauge.render_line(10);
        let total_chars: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total_chars, 10);

        let filled_chars: usize = line
            .spans
            .iter()
            .filter(|s| s.style == gauge.filled_style)
            .map(|s| s.content.chars().count())
            .sum();
        assert_eq!(filled_chars, 5);
    }

    #[test]
    fn test_gauge_custom_unfilled_char() {
        let gauge = Gauge::new().ratio(0.0).unfilled_char('.');
        let line = gauge.render_line(10);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains('.'));
        assert!(!text.contains('·'));
    }
}
