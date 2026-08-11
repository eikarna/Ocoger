//! Phase 5.2: Commands pane — list of .opencode/commands/*.md (name, description).

use crate::ui::app::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;

    // Header shows cursor count + uncommitted changes indicator.
    let header = Line::from(vec![
        Span::styled(
            format!(" {:3} ", app.commands.len()),
            Style::default().fg(t.dim),
        ),
        Span::raw("Commands "),
        if app.commands_is_dirty {
            Span::styled("[*]", Style::default().fg(t.warn))
        } else {
            Span::raw("")
        },
    ]);

    let items: Vec<ListItem> = app
        .commands
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let marker = if i == app.commands_cursor { "▌" } else { " " };
            let style = if i == app.commands_cursor {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {} ", i + 1)),
                Span::styled(format!("{:20}", &cmd.name[..cmd.name.len().min(19)]), style),
                Span::raw(" "),
                Span::styled(cmd.description.clone(), Style::default().fg(t.dim)),
            ]))
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let mut state = ListState::default();
    state.select(Some(app.commands_cursor));
    frame.render_stateful_widget(List::new(items), chunks[1], &mut state);

    // Footer with key hints for this pane.
    let hint = "[j/k] nav  [n] new command  [d] delete";
    let mut spans = header.spans;
    spans.push(Span::styled(
        format!("  {hint}"),
        Style::default().fg(t.dim),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), chunks[0]);
}
