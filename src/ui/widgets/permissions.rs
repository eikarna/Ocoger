//! Phase 5.5: Permissions pane — global permission.<tool> with per-agent overrides.

use crate::ui::app::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;

    let items: Vec<ListItem> = app
        .perm_rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let marker = if i == app.perm_cursor { "▌" } else { " " };
            let name_style = if i == app.perm_cursor {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            };
            let global_style = match r.global.as_str() {
                "allow" => Style::default().fg(t.accent),
                "deny" => Style::default().fg(t.warn),
                "ask" => Style::default().fg(t.syntax_keyword),
                _ => Style::default().fg(t.dim),
            };
            // Unset rows show the documented default, dimmed.
            let shown = if r.global.is_empty() {
                format!(
                    "({})",
                    crate::core::permissions::documented_default(&r.tool)
                )
            } else {
                r.global.clone()
            };
            let mut spans = vec![
                Span::raw(format!("{marker} {} ", i + 1)),
                Span::styled(format!("{:20}", r.tool), name_style),
                Span::raw(" "),
                Span::styled(format!("{:14}", shown), global_style),
                Span::raw(" "),
            ];
            for (agent, val) in &r.agent_overrides {
                spans.push(Span::styled(
                    format!("[{agent}:{val}] "),
                    Style::default().fg(t.syntax_keyword),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let mut state = ListState::default();
    state.select(Some(app.perm_cursor));
    frame.render_stateful_widget(List::new(items), chunks[1], &mut state);

    let hint = "[j/k] nav  [Space] allow/ask/deny  [e] agent override  [Esc] back";
    let header = Line::from(vec![
        Span::styled(
            format!(" {:3} ", app.perm_rows.len()),
            Style::default().fg(t.dim),
        ),
        Span::raw("Permissions "),
        Span::styled(format!("  {hint}"), Style::default().fg(t.dim)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);
}
