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
            // Fixed-width display-column fitting keeps long model ids from
            // overprinting the type and hint columns (the screenshot bug).
            let width = area.width.saturating_sub(8) as usize;
            let key_w = (width / 5).clamp(14, 18);
            let value_w = (width / 3).clamp(16, 36);
            let kind_w = 12;
            let hint_w = width.saturating_sub(key_w + value_w + kind_w);
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {:2} ", i + 1)),
                Span::styled(crate::ui::widgets::util::fit(&r.key, key_w), name_style),
                Span::styled(crate::ui::widgets::util::fit(&shown, value_w), value_style),
                Span::styled(
                    crate::ui::widgets::util::fit(kindtag, kind_w),
                    Style::default().fg(t.dim),
                ),
                Span::styled(
                    if selected {
                        crate::ui::widgets::util::clip(r.hint, hint_w)
                    } else {
                        String::new()
                    },
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
