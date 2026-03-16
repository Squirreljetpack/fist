use std::{collections::VecDeque, path::Path};

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
/// Processes a single line, manages the sliding window [before, after]
pub fn process_rg_line<F>(
    line: Line<'static>,
    ctx: [usize; 2], // [before, after]
    cwd: &Path,
    buffer: &mut VecDeque<BufItem>,
    mut on_item: F,
) -> anyhow::Result<()>
where
    F: FnMut(PathItem),
{
    let [before, after] = ctx;

    let Some((path, loc, mut text)) = parse_rg_line(line, ':', '-') else {
        return Ok(());
    };

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
    item.cmd = Some(match_item.loc.clone());

    let start = mid_idx.saturating_sub(before);
    let end = std::cmp::min(buffer.len(), mid_idx + after + 1);

    let mut lines = Vec::new();
    for b in buffer.iter().take(end).skip(start) {
        lines.push(b.line.clone());
    }

    item.tail = Text::from(lines);
    on_item(item);
}
