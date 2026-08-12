//! Phase 5.4: MCP servers pane — mcp.<name> toggles/edits.

use crate::ui::app::App;
use crate::ui::widgets::util::{clip, fit};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;

    let items: Vec<ListItem> = app
        .mcp_list
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let marker = if i == app.mcp_cursor { "▌" } else { " " };
            let state = if e.enabled { "●" } else { "○" };
            let state_style = if e.enabled {
                Style::default().fg(t.accent)
            } else {
                Style::default().fg(t.dim)
            };
            let name_style = if i == app.mcp_cursor {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            };
            let usable = area.width.saturating_sub(10) as usize;
            let name_w = (usable / 4).clamp(14, 26);
            let kind = format!("({})", e.kind);
            let value_w = usable.saturating_sub(name_w + kind.chars().count() + 3);
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {:2} ", i + 1)),
                Span::styled(state, state_style),
                Span::raw(" "),
                Span::styled(fit(&e.name, name_w), name_style),
                Span::styled(fit(&kind, 10), Style::default().fg(t.dim)),
                Span::styled(
                    clip(&e.command_or_url, value_w),
                    Style::default().fg(t.syntax_keyword),
                ),
            ]))
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let mut state = ListState::default();
    state.select(Some(app.mcp_cursor));
    frame.render_stateful_widget(List::new(items), chunks[1], &mut state);

    let hint = "[Space] enable  [e] url/command  [t] type  [d] delete  [Esc] back";
    let header = Line::from(vec![
        Span::styled(
            format!(" {:3} ", app.mcp_list.len()),
            Style::default().fg(t.dim),
        ),
        Span::raw("MCP Servers "),
        Span::styled(format!("  {hint}"), Style::default().fg(t.dim)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);
}
