//! Phase 5.3: Providers & Models pane — list providers from merged opencode.json(c).

use crate::ui::app::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;

    // Header shows count + dirty indicator.
    let header = Line::from(vec![
        Span::styled(
            format!(" {:3} ", app.providers_list.len()),
            Style::default().fg(t.dim),
        ),
        Span::raw("Providers "),
        if app.providers_is_dirty {
            Span::styled("[*]", Style::default().fg(t.warn))
        } else {
            Span::raw("")
        },
    ]);

    let items: Vec<ListItem> = app
        .providers_list
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let marker = if i == app.providers_cursor {
                "▌"
            } else {
                " "
            };
            let style = if i == app.providers_cursor {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            };
            let name = format!("{:20}", p.name.as_deref().unwrap_or(&p.id));
            let url = p.base_url.as_deref().unwrap_or("<none>");
            let key_flag = if p.has_api_key_ref { "[env:key]" } else { "" };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{marker} {} ", i + 1)),
                Span::styled(name, style),
                Span::raw(" "),
                Span::styled(url, Style::default().fg(t.syntax_keyword)),
                Span::raw(format!(" {} ", key_flag)),
            ]))
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let mut state = ListState::default();
    state.select(Some(app.providers_cursor));
    frame.render_stateful_widget(List::new(items), chunks[1], &mut state);

    // Footer with key hints.
    let hint = "[j/k] nav  [e] baseURL  [a] apiKey  [d] delete  [Esc] back";
    // Header line: count + dirty flag, then key hints.
    let mut spans = header.spans;
    spans.push(Span::styled(
        format!("  {hint}"),
        Style::default().fg(t.dim),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), chunks[0]);
}
