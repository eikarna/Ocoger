//! Permission rows, including nested glob/command pattern rows.

use crate::ui::app::App;
use crate::ui::widgets::util::{clip, fit};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let usable = area.width.saturating_sub(8) as usize;
    let label_w = (usable / 3).clamp(16, 30);
    let value_w = (usable / 4).clamp(12, 22);

    let items: Vec<ListItem> = app
        .perm_rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let selected = i == app.perm_cursor;
            let marker = if selected { "▌" } else { " " };
            let name_style = if selected {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else if r.depth == 1 {
                Style::default().fg(t.dim)
            } else {
                Style::default().fg(t.fg)
            };
            let shown = r.effective();
            let value_style = match r.value.as_str() {
                "allow" => Style::default().fg(t.accent),
                "deny" => Style::default().fg(t.warn),
                "ask" => Style::default().fg(t.syntax_keyword),
                _ => Style::default().fg(t.dim),
            };
            let mut label = if r.depth == 1 {
                format!("  ↳ {}", r.label)
            } else {
                r.label.clone()
            };
            if let Some(agent) = &r.agent {
                label = format!("{agent}: {label}");
            }
            let suffix = if r.is_container() {
                format!("{} rules; [n] add", r.pattern_count)
            } else if r.depth == 1 {
                "[e] edit  [d] delete".to_string()
            } else {
                "[Space] cycle  [e] edit".to_string()
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {:2} ", i + 1)),
                Span::styled(fit(&label, label_w), name_style),
                Span::styled(fit(&shown, value_w), value_style),
                Span::styled(
                    clip(&suffix, usable.saturating_sub(label_w + value_w)),
                    Style::default().fg(t.dim),
                ),
            ]))
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);
    let mut state = ListState::default();
    state.select(Some(app.perm_cursor));
    frame.render_stateful_widget(List::new(items), chunks[1], &mut state);
    let header = Line::from(vec![
        Span::styled(
            format!(" {:3} ", app.perm_rows.len()),
            Style::default().fg(t.dim),
        ),
        Span::raw("Permissions "),
        Span::styled(
            "[j/k] nav  [Space] cycle  [e] edit  [n] pattern  [d] delete  [Esc] back",
            Style::default().fg(t.dim),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);
}
