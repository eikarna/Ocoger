//! Phase 5.6: Process & Logs pane — supervised opencode status + tail.

use crate::ui::app::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    let state_style = match app.proc_state {
        crate::services::process_manager::ProcState::Running => {
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
        }
        crate::services::process_manager::ProcState::Restarting => Style::default().fg(t.warn),
        _ => Style::default().fg(t.dim),
    };
    let header = Line::from(vec![
        Span::raw("Process "),
        Span::styled(format!("{:?}", app.proc_state), state_style),
        Span::raw(" "),
        Span::styled(
            app.proc_pid
                .map(|p| format!("pid={}", p))
                .unwrap_or_default(),
            Style::default().fg(t.dim),
        ),
        Span::raw("   "),
        Span::styled(
            "[S] start  [X] kill  [R] restart  [j/k] scroll tail",
            Style::default().fg(t.dim),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);

    let items: Vec<ListItem> = app
        .process_buf
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let style = if l.starts_with("[stderr]") {
                Style::default().fg(t.warn)
            } else if l.starts_with("[stdout]") {
                Style::default().fg(t.fg)
            } else {
                Style::default().fg(t.dim)
            };
            ListItem::new(Line::from(vec![Span::styled(
                format!("{:4} {}", i + 1, l),
                style,
            )]))
        })
        .collect();
    let visible = chunks[1].height.saturating_sub(2) as usize;
    let skip = app
        .proc_scroll
        .saturating_add(visible as u16)
        .saturating_sub(1)
        .saturating_sub(app.process_buf.len() as u16) as usize;
    let _ = skip;
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" tail ({} lines) ", app.process_buf.len())),
    );
    let mut state = ListState::default();
    if !app.process_buf.is_empty() {
        // Cursor tracks which buffered line we show at the top.
        let idx = app
            .process_buf
            .len()
            .saturating_sub(1 + app.proc_scroll as usize);
        state.select(Some(idx.min(app.process_buf.len() - 1)));
    }
    frame.render_stateful_widget(list, chunks[1], &mut state);
}
