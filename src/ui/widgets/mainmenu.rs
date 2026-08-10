//! Phase 5 hub: Main Menu pane. Rendered at boot; Enter/digit dispatches
//! into a leaf pane, Esc/q quits. Theme-aware like the other widgets.

use crate::ui::app::{App, MAINMENU_ITEMS};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(3),
            ratatui::layout::Constraint::Min(1),
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " OpenCode Manager ",
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "— pick a pane (Enter / 1-6; Esc/q quits)",
            Style::default().fg(t.dim),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(t.border)),
    );
    frame.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = MAINMENU_ITEMS
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| {
            let marker = if i == app.mainmenu_cursor { "▌" } else { " " };
            let style = if i == app.mainmenu_cursor {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {}. ", i + 1)),
                Span::styled(format!("{name:20}"), style),
                Span::styled(*desc, Style::default().fg(t.dim)),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.mainmenu_cursor));
    frame.render_stateful_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(t.border))
                .title(" panes "),
        ),
        chunks[1],
        &mut state,
    );
}
