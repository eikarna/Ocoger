//! Agent list pane: checkbox-style multi-select, model + dirty [*] badge.

use crate::core::agent_parser::AgentFile;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: Rect, agents: &[AgentFile], cursor: usize) {
    let header = Line::from(vec![
        Span::raw("   sel  "),
        Span::styled("agent.md", Style::default().fg(Color::Gray)),
        Span::raw("  "),
        Span::styled("current model", Style::default().fg(Color::Gray)),
    ]);
    let mut items: Vec<ListItem> = vec![ListItem::new(header)];

    for (i, a) in agents.iter().enumerate() {
        let check = if a.is_selected { "[x]" } else { "[ ]" };
        let dirty = if a.is_dirty { "[*]" } else { "   " };
        let marker = if i == cursor { ">" } else { " " };
        let name = a
            .path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| a.path.display().to_string());
        let style = if i == cursor {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        items.push(ListItem::new(Line::from(vec![
            Span::raw(format!("{marker} {check} {dirty} ")),
            Span::styled(format!("{name:18}"), style),
            Span::raw(a.frontmatter.model.clone()),
        ])));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" SUBAGENTS (.opencode/agents/*.md) ");

    if agents.is_empty() {
        frame.render_widget(
            Paragraph::new("No agents found. Drop .md files into .opencode/agents/").block(block),
            area,
        );
        return;
    }

    let mut state = ListState::default();
    // +1 for the header row.
    state.select(Some(cursor + 1));
    frame.render_stateful_widget(List::new(items).block(block), area, &mut state);
}

/// Batch model editor modal (FE-1.3).
pub fn render_modal(frame: &mut Frame, area: Rect, input: &str, target_count: usize) {
    let w = 60u16;
    let h = 5u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    let text = vec![
        Line::from(format!("Apply model to {target_count} selected agent(s):")),
        Line::from(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(input, Style::default().fg(Color::Cyan)),
            Span::styled("_", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Change Model "),
            )
            .wrap(Wrap { trim: true }),
        popup,
    );
}

/// Bottom bar: rolling log lines + key hints.
pub fn render_bottom(frame: &mut Frame, area: Rect, log: &[String]) {
    // Split into log (grow) + hint (1 line).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    frame.render_widget(
        Paragraph::new(
            log.iter()
                .map(|m| Line::from(m.clone()))
                .collect::<Vec<_>>(),
        )
        .block(Block::default().borders(Borders::ALL).title(" log ")),
        chunks[0],
    );

    let hint = "[j/k] nav  [Space] tag  [a] all  [m] model  [s] save  [q] quit";
    frame.render_widget(Paragraph::new(hint), chunks[1]);
}
