//! Phase 5.2: Commands pane — list of .opencode/commands/*.md (name, description).

use crate::ui::app::App;
use crate::ui::widgets::util::{clip, fit};
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
            let usable = area.width.saturating_sub(8) as usize;
            let name_w = (usable / 3).clamp(16, 28);
            let meta = match (&cmd.agent, &cmd.model) {
                (Some(agent), Some(model)) => format!(" [{agent} · {model}]"),
                (Some(agent), None) => format!(" [{agent}]"),
                (None, Some(model)) => format!(" [{model}]"),
                (None, None) => String::new(),
            };
            let desc_w = usable.saturating_sub(name_w + meta.chars().count().min(28));
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {:2} ", i + 1)),
                Span::styled(fit(&cmd.name, name_w), style),
                Span::styled(clip(&cmd.description, desc_w), Style::default().fg(t.dim)),
                Span::styled(clip(&meta, 28), Style::default().fg(t.syntax_keyword)),
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
    let hint = "[j/k] nav  [e] description  [a] agent  [m] model  [n] new  [d] delete";
    let mut spans = header.spans;
    spans.push(Span::styled(
        format!("  {hint}"),
        Style::default().fg(t.dim),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), chunks[0]);
}
