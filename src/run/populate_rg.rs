use std::{collections::VecDeque, path::Path};

use ansi_to_tui::IntoText;
use ratatui::text::{Line, Text};

use crate::{
    run::item::PathItem,
    utils::text::{parse_rg_line, scrub_text_styles},
};

#[derive(Clone, Debug)]
pub struct BufItem {
    pub path: String,
    pub loc: String,
    pub line: Line<'static>,
    pub is_match: bool,
}
/// Processes a single line, managing the sliding window [before, after]
/// with optional path prefix support for multiline filenames.
pub fn process_rg_line<F>(
    line: Line<'static>,
    path_prefix: Option<&str>,
    ctx: [usize; 2], // [before, after]
    cwd: &Path,
    no_column: bool,
    buffer: &mut VecDeque<BufItem>,
    mut on_item: F,
) -> anyhow::Result<()>
where
    F: FnMut(PathItem),
{
    let [before, after] = ctx;

    let Some((mut path, loc, mut text)) = parse_rg_line(line, ':', '-', no_column) else {
        return Ok(());
    };

    if let Some(prefix) = path_prefix {
        if !prefix.is_empty() {
            path = format!("{}{}", prefix, path);
        }
    }

    scrub_text_styles(&mut text);
    let is_match = loc.ends_with(':');
    buffer.push_back(BufItem {
        path,
        loc,
        line: text.lines.remove(0),
        is_match,
    });

    // 3. Maintenance: Pop if we exceeded maximum possible window size
    // Max size needed is 'before' lines + the match + 'after' lines
    if buffer.len() > (before + after + 1) {
        // Only pop if the front isn't a match still waiting for its own 'after' context
        // But for a simple sliding window, we keep the last (B+A+1) lines.
        if buffer.len() > (before + after + 1) {
            buffer.pop_front();
        }
    }

    // 4. Check if a match has reached the "stable" point in the buffer
    // A match is ready when it is at index: buffer.len() - 1 - after
    if buffer.len() > after {
        let mid_idx = buffer.len() - 1 - after;
        if buffer[mid_idx].is_match {
            push_match_from_buffer(mid_idx, ctx, cwd, buffer, &mut on_item);
        }
    }

    Ok(())
}

pub fn flush_rg_buffer<F>(
    ctx: [usize; 2],
    cwd: &Path,
    buffer: &mut VecDeque<BufItem>,
    mut on_item: F,
) where
    F: FnMut(PathItem),
{
    let [_before, after] = ctx;
    let len = buffer.len();
    if len == 0 {
        return;
    }

    // On flush, any matches in the last 'after' lines didn't get their full
    // context, but we push them now anyway.
    let start = len.saturating_sub(after);
    for i in start..len {
        if buffer[i].is_match {
            push_match_from_buffer(i, ctx, cwd, buffer, &mut on_item);
        }
    }
    buffer.clear();
}

fn push_match_from_buffer<F>(
    mid_idx: usize,
    ctx: [usize; 2],
    cwd: &Path,
    buffer: &VecDeque<BufItem>,
    on_item: &mut F,
) where
    F: FnMut(PathItem),
{
    let [before, after] = ctx;
    let match_item = &buffer[mid_idx];
    let mut item = PathItem::new(match_item.path.clone(), cwd);
    if let Some((line, col)) = crate::utils::text::parse_loc(&match_item.loc) {
        item.set_loc(line, col);
    }

    let start = mid_idx.saturating_sub(before);
    let end = std::cmp::min(buffer.len(), mid_idx + after + 1);

    let mut lines = Vec::new();
    for b in buffer.iter().take(end).skip(start) {
        lines.push(b.line.clone());
    }

    item.tail = Err(Text::from(lines));
    on_item(item);
}

/// Streaming parser for ripgrep's multi-line (`--heading --null`) mode.
/// Groups all match and context lines for each file into a single `PathItem`,
/// properly supporting filepaths containing embedded newlines.
#[derive(Default, Debug)]
pub struct MultilineRgParser {
    pub current_path: String,
    pub path_buffer: String,
    pub current_context: Vec<Line<'static>>,
    pub current_places: String,
}

impl MultilineRgParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process_line<F>(
        &mut self,
        line: String,
        cwd: &Path,
        no_column: bool,
        vis: fist_types::filters::Visibility,
        on_item: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(PathItem),
    {
        if self.current_path.is_empty() {
            if let Some((p, rest)) = line.split_once('\0') {
                let full_path = if self.path_buffer.is_empty() {
                    p.to_string()
                } else {
                    let mut s = std::mem::take(&mut self.path_buffer);
                    s.push_str(p);
                    s
                };

                let path_str = full_path
                    .as_bytes()
                    .into_text()
                    .map(|x| crate::utils::text::text_to_string(&x))
                    .unwrap_or(full_path);

                if !path_str.is_empty() {
                    self.current_path = path_str;
                }

                if !rest.is_empty() {
                    let text = rest
                        .as_bytes()
                        .into_text()
                        .unwrap_or_else(|_| Text::from_iter([rest.to_string()]));
                    self.current_context.extend(text.lines);
                }
            } else {
                self.path_buffer.push_str(&line);
                self.path_buffer.push('\n');
            }
            Ok(())
        } else if line.is_empty() {
            self.flush(cwd, no_column, vis, on_item)
        } else {
            let text = line
                .as_bytes()
                .into_text()
                .unwrap_or_else(|_| Text::from_iter([line]));
            self.current_context.extend(text.lines);
            Ok(())
        }
    }

    pub fn flush<F>(
        &mut self,
        cwd: &Path,
        no_column: bool,
        vis: fist_types::filters::Visibility,
        mut on_item: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(PathItem),
    {
        if self.current_path.is_empty() {
            self.path_buffer.clear();
            self.current_context.clear();
            self.current_places.clear();
            return Ok(());
        }

        let mut item = PathItem::new(std::mem::take(&mut self.current_path), cwd);
        let mut text = Text::from(std::mem::take(&mut self.current_context));
        scrub_text_styles(&mut text);
        for l in &text.lines {
            crate::utils::text::extract_rg_line_no_path(l, &mut self.current_places, no_column);
        }

        item.tail = Err(text);
        if let Some((line, col)) = crate::utils::text::first_loc(&self.current_places) {
            item.set_loc(line, col);
        }
        self.current_places.clear();

        if vis.post_fd_filter(&item.path) {
            on_item(item);
        }
        Ok(())
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use ansi_to_tui::IntoText;
    use std::path::PathBuf;

    #[test]
    fn test_process_rg_line_and_loc() {
        let cwd = PathBuf::from("/test");
        let raw = "\x1b[0m\x1b[35mfoo.rs\x1b[0m\0\x1b[0m\x1b[32m42\x1b[0m:\x1b[0m10\x1b[0m:pub fn test() {";
        let text = raw.as_bytes().into_text().unwrap();
        let mut buffer = VecDeque::new();
        let mut items = Vec::new();

        process_rg_line(
            text.lines.into_iter().next().unwrap(),
            None,
            [0, 0],
            &cwd,
            false,
            &mut buffer,
            |item| items.push(item),
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.loc(), (42, 10));
        assert_eq!(item.tail_text().to_string(), "pub fn test() {");
    }

    #[test]
    fn test_process_rg_line_with_context() {
        let cwd = PathBuf::from("/test");
        let raw_ctx_before = "\x1b[0m\x1b[35mfoo.rs\x1b[0m\0\x1b[0m\x1b[32m41\x1b[0m-\x1b[0m// comment";
        let raw_match = "\x1b[0m\x1b[35mfoo.rs\x1b[0m\0\x1b[0m\x1b[32m42\x1b[0m:\x1b[0m10\x1b[0m:pub fn test() {";
        let raw_ctx_after = "\x1b[0m\x1b[35mfoo.rs\x1b[0m\0\x1b[0m\x1b[32m43\x1b[0m-\x1b[0m}";

        let t1 = raw_ctx_before.as_bytes().into_text().unwrap();
        let t2 = raw_match.as_bytes().into_text().unwrap();
        let t3 = raw_ctx_after.as_bytes().into_text().unwrap();

        let mut buffer = VecDeque::new();
        let mut items = Vec::new();

        process_rg_line(
            t1.lines.into_iter().next().unwrap(),
            None,
            [1, 1],
            &cwd,
            false,
            &mut buffer,
            |item| items.push(item),
        )
        .unwrap();
        assert_eq!(items.len(), 0);

        process_rg_line(
            t2.lines.into_iter().next().unwrap(),
            None,
            [1, 1],
            &cwd,
            false,
            &mut buffer,
            |item| items.push(item),
        )
        .unwrap();
        assert_eq!(items.len(), 0);

        process_rg_line(
            t3.lines.into_iter().next().unwrap(),
            None,
            [1, 1],
            &cwd,
            false,
            &mut buffer,
            |item| items.push(item),
        )
        .unwrap();
        assert_eq!(items.len(), 1);

        let item = &items[0];
        assert_eq!(item.loc(), (42, 10));
        assert_eq!(item.tail_text().lines.len(), 3);
    }

    #[test]
    fn test_multiline_rg_parser_single_file() {
        let cwd = PathBuf::from("/workspace");
        let mut parser = MultilineRgParser::new();
        let mut items = Vec::new();
        let vis = fist_types::filters::Visibility::default();

        let lines = vec![
            "src/run/start.rs\028:20:        previewer::make_previewer,".to_string(),
            "253:21:    let previewer = make_previewer(".to_string(),
            "".to_string(),
        ];

        for line in lines {
            parser
                .process_line(line, &cwd, false, vis, |item| items.push(item))
                .unwrap();
        }

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(
            item.path.inner(),
            PathBuf::from("/workspace/src/run/start.rs")
        );
        assert_eq!(item.loc(), (28, 20));
        let tail_str = item.tail_text().to_string();
        assert!(tail_str.contains("previewer::make_previewer,"));
        assert!(tail_str.contains("let previewer = make_previewer("));
        assert_eq!(item.tail_text().lines.len(), 2);
    }

    #[test]
    fn test_multiline_rg_parser_multiple_files_with_ansi_and_context() {
        let cwd = PathBuf::from("/workspace");
        let mut parser = MultilineRgParser::new();
        let mut items = Vec::new();
        let vis = fist_types::filters::Visibility::default();

        let lines = vec![
            // File 1 with ANSI formatting, null byte separator, context line, and two match lines
            "\x1b[0m\x1b[35msrc/run/start.rs\x1b[0m\0\x1b[0m\x1b[32m27\x1b[0m-\x1b[0m// queue::QUEUE,".to_string(),
            "\x1b[0m\x1b[32m28\x1b[0m:\x1b[0m20\x1b[0m:previewer::make_previewer,".to_string(),
            "\x1b[0m\x1b[32m253\x1b[0m:\x1b[0m21\x1b[0m:let previewer = make_previewer(".to_string(),
            "".to_string(),
            // File 2 without trailing empty line (flushed at EOF)
            "\x1b[0m\x1b[35msrc/run/previewer.rs\x1b[0m\0\x1b[0m\x1b[32m18\x1b[0m:\x1b[0m8\x1b[0m:pub fn make_previewer(".to_string(),
        ];

        for line in lines {
            parser
                .process_line(line, &cwd, false, vis, |item| items.push(item))
                .unwrap();
        }

        // EOF flush
        parser
            .flush(&cwd, false, vis, |item| items.push(item))
            .unwrap();

        assert_eq!(items.len(), 2);

        // Verify File 1
        let item1 = &items[0];
        assert_eq!(
            item1.path.inner(),
            PathBuf::from("/workspace/src/run/start.rs")
        );
        assert_eq!(item1.loc(), (28, 20));
        assert_eq!(item1.tail_text().lines.len(), 3);
        let tail1 = item1.tail_text().to_string();
        assert!(tail1.contains("// queue::QUEUE,"));
        assert!(tail1.contains("previewer::make_previewer,"));
        assert!(tail1.contains("let previewer = make_previewer("));

        // Verify File 2
        let item2 = &items[1];
        assert_eq!(
            item2.path.inner(),
            PathBuf::from("/workspace/src/run/previewer.rs")
        );
        assert_eq!(item2.loc(), (18, 8));
        assert_eq!(item2.tail_text().lines.len(), 1);
        let tail2 = item2.tail_text().to_string();
        assert!(tail2.contains("pub fn make_previewer("));
    }

    #[test]
    fn test_multiline_filepath_oneline_mode() {
        let cwd = PathBuf::from("/workspace");
        let raw_line = "\x1b[0m\x1b[35mbaz.rs\x1b[0m\0\x1b[0m\x1b[32m42\x1b[0m:\x1b[0m10\x1b[0m:pub fn test() {";
        let text = raw_line.as_bytes().into_text().unwrap();
        let mut buffer = VecDeque::new();
        let mut items = Vec::new();

        // Path buffer accumulated the preceding lines of a multiline filename
        let path_prefix = "foo\nbar/";

        process_rg_line(
            text.lines.into_iter().next().unwrap(),
            Some(path_prefix),
            [0, 0],
            &cwd,
            false,
            &mut buffer,
            |item| items.push(item),
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(
            item.path.inner(),
            PathBuf::from("/workspace/foo\nbar/baz.rs")
        );
        assert_eq!(item.loc(), (42, 10));
        assert_eq!(item.tail_text().to_string(), "pub fn test() {");
    }

    #[test]
    fn test_multiline_filepath_non_oneline_mode() {
        let cwd = PathBuf::from("/workspace");
        let mut parser = MultilineRgParser::new();
        let mut items = Vec::new();
        let vis = fist_types::filters::Visibility::default();

        let lines = vec![
            // File 1: filepath contains embedded newlines across 3 lines
            "\x1b[0m\x1b[35mdeep/nested\x1b[0m".to_string(),
            "\x1b[0m\x1b[35mfolder/my\x1b[0m".to_string(),
            "\x1b[0m\x1b[35mfile.rs\x1b[0m\0\x1b[0m\x1b[32m10\x1b[0m:\x1b[0m5\x1b[0m:first line".to_string(),
            "\x1b[0m\x1b[32m11\x1b[0m:\x1b[0m5\x1b[0m:second line".to_string(),
            "".to_string(),
            // File 2: filepath contains embedded newline across 2 lines, ended by EOF
            "\x1b[0m\x1b[35manother/sub\x1b[0m".to_string(),
            "\x1b[0m\x1b[35mpath.rs\x1b[0m\0\x1b[0m\x1b[32m99\x1b[0m:\x1b[0m1\x1b[0m:final match".to_string(),
        ];

        for line in lines {
            parser
                .process_line(line, &cwd, false, vis, |item| items.push(item))
                .unwrap();
        }

        parser
            .flush(&cwd, false, vis, |item| items.push(item))
            .unwrap();

        assert_eq!(items.len(), 2);

        let item1 = &items[0];
        assert_eq!(
            item1.path.inner(),
            PathBuf::from("/workspace/deep/nested\nfolder/my\nfile.rs")
        );
        assert_eq!(item1.loc(), (10, 5));
        assert_eq!(item1.tail_text().lines.len(), 2);
        let tail1 = item1.tail_text().to_string();
        assert!(tail1.contains("first line"));
        assert!(tail1.contains("second line"));

        let item2 = &items[1];
        assert_eq!(
            item2.path.inner(),
            PathBuf::from("/workspace/another/sub\npath.rs")
        );
        assert_eq!(item2.loc(), (99, 1));
        assert_eq!(item2.tail_text().lines.len(), 1);
        let tail2 = item2.tail_text().to_string();
        assert!(tail2.contains("final match"));
    }
}



