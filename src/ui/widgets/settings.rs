//! Phase 5.7: Settings pane — top-level scalars from the OpenCode v1 schema.

use crate::core::settings::Kind;
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
            let selected = i == app.settings_cursor;
            let marker = if selected { "▌" } else { " " };
            let name_style = if selected {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            };
            // Unset keys show the documented default, dimmed, so the pane never
            // implies a value the config file doesn't actually contain.
            let (shown, value_style) = if r.value.is_empty() {
                let d = if r.default.is_empty() {
                    "—".to_string()
                } else {
                    format!("({})", r.default)
                };
                (d, Style::default().fg(t.dim))
            } else {
                let style = match r.value.as_str() {
                    "true" => Style::default().fg(t.accent),
                    "false" | "disabled" => Style::default().fg(t.dim),
                    "notify" | "ask" | "manual" => Style::default().fg(t.syntax_keyword),
                    _ => Style::default().fg(t.syntax_keyword),
                };
                (r.value.clone(), style)
            };
            let kindtag = match r.kind {
                Kind::Text => "text",
                Kind::Bool => "bool",
                Kind::BoolOrNotify => "bool|notify",
                Kind::Enum(_) => "enum",
                Kind::Number => "num",
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {} ", i + 1)),
                Span::styled(format!("{:16}", r.key), name_style),
                Span::styled(format!("{:14}", shown), value_style),
                Span::styled(format!("{:12}", kindtag), Style::default().fg(t.dim)),
                Span::styled(
                    if selected { r.hint } else { "" },
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
    state.select(Some(app.settings_cursor));
    frame.render_stateful_widget(List::new(items), chunks[1], &mut state);

    let hint = "[j/k] nav  [Space] toggle/cycle  [e] edit text  [Esc] back";
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
