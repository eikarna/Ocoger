//! Shared widget helpers: column-safe truncation and the reusable edit prompt.

use crate::ui::app::{App, EditKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Fit `s` into `width` display columns, ellipsising the middle so both the
/// start (which identifies the value) and the tail (often the distinguishing
/// part, e.g. a model suffix) stay readable. Padded to exactly `width` so the
/// next column always lands on the same offset.
pub fn fit(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n <= width {
        return format!("{s:width$}");
    }
    if width <= 1 {
        return "…".repeat(width);
    }
    let keep = width - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let chars: Vec<char> = s.chars().collect();
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[n - tail..]);
    out
}

/// Truncate without padding, for trailing columns that shouldn't be padded out.
pub fn clip(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n <= width {
        return s.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out: String = chars[..width - 1].iter().collect();
    out.push('…');
    out
}

/// Centered popup rect, clamped to the frame.
pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// The single edit dialog used by every pane that changes a value. Renders the
/// full key path, the editable buffer with a cursor, and the commit/cancel
/// keys. Enum-valued prompts additionally list the allowed literals.
pub fn edit_prompt(frame: &mut Frame, area: Rect, app: &App) {
    let Some(p) = app.edit_prompt.as_ref() else {
        return;
    };
    let t = &app.theme;
    let allowed: Option<String> = match p.kind {
        EditKind::Enum(vals) => Some(vals.join(" | ")),
        _ => None,
    };
    let height = if allowed.is_some() { 8 } else { 7 };
    let popup = centered(area, 78, height);

    let mut lines = vec![
        Line::from(Span::styled(
            clip(&p.keypath.join("."), popup.width.saturating_sub(4) as usize),
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            clip(p.hint, popup.width.saturating_sub(4) as usize),
            Style::default().fg(t.dim),
        )),
        Line::from(vec![
            Span::styled("current: ", Style::default().fg(t.dim)),
            Span::styled(
                if p.current.is_empty() {
                    "(unset)"
                } else {
                    &p.current
                },
                Style::default().fg(t.syntax_keyword),
            ),
        ]),
    ];
    if let Some(a) = allowed {
        lines.push(Line::from(vec![
            Span::styled("allowed: ", Style::default().fg(t.dim)),
            Span::styled(a, Style::default().fg(t.syntax_keyword)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("> ", Style::default().fg(t.accent)),
        Span::styled(p.buffer.clone(), Style::default().fg(t.fg)),
        // Block cursor so the focus is unmistakable.
        Span::styled(" ", Style::default().bg(t.accent)),
    ]));
    lines.push(Line::from(Span::styled(
        "[Enter] save   [Esc] cancel   [Backspace] erase",
        Style::default().fg(t.dim),
    )));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(t.accent))
                .title(format!(" edit {} ", p.title)),
        ),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_pads_short_and_middle_ellipsises_long() {
        assert_eq!(
            fit("ab", 5),
            "ab   ",
            "short values pad to the column width"
        );
        assert_eq!(fit("abcde", 5), "abcde", "exact fit is untouched");
        // Long values keep head and tail so the next column still aligns.
        let out = fit("9router/cf/@cf/qwen/qwen2.5-coder-32b-instruct", 14);
        assert_eq!(out.chars().count(), 14, "never exceeds the column");
        assert!(out.contains('…'));
        assert!(out.starts_with("9router"), "head preserved");
        assert!(
            out.ends_with("truct"),
            "tail preserved within narrow column"
        );
    }

    #[test]
    fn clip_truncates_with_ellipsis_and_no_padding() {
        assert_eq!(clip("abc", 10), "abc", "no padding added");
        assert_eq!(clip("abcdefgh", 4), "abc…");
        assert_eq!(clip("abc", 0), "");
    }
}
