//! Phase 5.7: Settings pane — theme / default_agent / autoupdate / share.

use crate::ui::app::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;

    let items: Vec<ListItem> = app
        .settings_rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let marker = if i == app.settings_cursor { "▌" } else { " " };
            let name_style = if i == app.settings_cursor {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            };
            let value_style = match r.value.as_str() {
                "true" | "on" => Style::default().fg(t.accent),
                "false" | "off" => Style::default().fg(t.dim),
                "ask" => Style::default().fg(t.syntax_keyword),
                v if v.is_empty() => Style::default().fg(t.dim),
                _ => Style::default().fg(t.syntax_keyword),
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {} ", i + 1)),
                Span::styled(format!("{:18}", r.key), name_style),
                Span::raw(" "),
                Span::styled(
                    if r.value.is_empty() {
                        "—".into()
                    } else {
                        r.value.clone()
                    },
                    value_style,
                ),
            ]))
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let mut state = ListState::default();
    state.select(Some(app.settings_cursor));
    frame.render_stateful_widget(List::new(items), chunks[1], &mut state);

    let hint = "[Space] toggle bool/cycle  [e] edit string  [Esc] back";
    let header = Line::from(vec![
        Span::styled(
            format!(" {:3} ", app.settings_rows.len()),
            Style::default().fg(t.dim),
        ),
        Span::raw("Settings "),
        Span::styled(format!("  {hint}"), Style::default().fg(t.dim)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);
}
