//! Phase 5.4: MCP servers pane — mcp.<name> toggles/edits.

use crate::ui::app::App;
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
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {} ", i + 1)),
                Span::styled(state, state_style),
                Span::raw(" "),
                Span::styled(format!("{:22}", e.name), name_style),
                Span::raw(" "),
                Span::styled(format!("({})", e.kind), Style::default().fg(t.dim)),
                Span::raw(" "),
                Span::styled(
                    e.command_or_url.clone(),
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

    let hint = "[Space] enable/disable  [t] local/remote  [d] delete  [Esc] back";
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
