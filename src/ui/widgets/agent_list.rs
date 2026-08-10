//! Agent list pane: checkbox-style multi-select, model + dirty [*] badge.
//! Theme-aware — reads `app.theme` so color slots follow the user's palette.

use crate::ui::app::App;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let agents = &app.agents;
    let cursor = app.cursor;

    let header = Line::from(vec![
        Span::raw("   sel  "),
        Span::styled("agent.md", Style::default().fg(t.dim)),
        Span::raw("  "),
        Span::styled("current model", Style::default().fg(t.dim)),
    ]);
    let mut items: Vec<ListItem> = vec![ListItem::new(header)];

    for (i, a) in agents.iter().enumerate() {
        let check = if a.is_selected { "[x]" } else { "[ ]" };
        let dirty = if a.is_dirty { "[*]" } else { "   " };
        // Tiny scope marker (⊢ project, ⊢G global) after the filename.
        let scope = match a.origin {
            crate::core::agent_parser::AgentOrigin::Project => "  P",
            crate::core::agent_parser::AgentOrigin::Global => "  G",
        };
        let scope_style = match a.origin {
            crate::core::agent_parser::AgentOrigin::Project => Style::default().fg(t.accent),
            crate::core::agent_parser::AgentOrigin::Global => Style::default().fg(t.dim),
        };
        let marker = if i == cursor { "▌" } else { " " };
        let name = a
            .path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| a.path.display().to_string());
        let style = if i == cursor {
            Style::default()
                .fg(t.accent)
                .bg(t.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.fg)
        };
        let badge_style = if a.is_dirty {
            Style::default().fg(t.warn)
        } else {
            Style::default().fg(t.dim)
        };
        items.push(ListItem::new(Line::from(vec![
            Span::raw(format!("{marker} {check} ")),
            Span::styled(dirty, badge_style),
            Span::styled(format!(" {name:18}"), style),
            Span::styled(scope, scope_style),
            Span::raw(" "),
            Span::styled(a.frontmatter.model.clone(), Style::default().fg(t.dim)),
        ])));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.border))
        .title(" SUBAGENTS (.opencode/agents/*.md) ");

    if agents.is_empty() {
        frame.render_widget(
            Paragraph::new("No agents found. Drop .md files into .opencode/agents/").block(block),
            area,
        );
        return;
    }

    let mut state = ListState::default();
    state.select(Some(cursor + 1));
    frame.render_stateful_widget(List::new(items).block(block), area, &mut state);
}

/// Batch model editor modal (FE-1.3).
pub fn render_modal(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let input = &app.modal_input;
    let target_count = app.selected_count();

    let w = 60u16;
    let h = 5u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    let text = vec![
        Line::from(format!("Apply model to {target_count} selected agent(s):")),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(t.accent)),
            Span::styled(input, Style::default().fg(t.syntax_keyword)),
            Span::styled("█", Style::default().fg(t.dim)),
        ]),
    ];

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(t.accent))
                    .title(" Change Model "),
            )
            .wrap(Wrap { trim: true }),
        popup,
    );
}

/// Bottom bar: rolling log lines + key hints.
pub fn render_bottom(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let log_lines: Vec<Line> = app
        .log
        .iter()
        .map(|m| {
            let style = if m.contains("ERROR") || m.contains("FAIL") {
                Style::default().fg(t.warn)
            } else if m.starts_with("[model-fetch]") || m.contains("merged") {
                Style::default().fg(t.syntax_keyword)
            } else {
                Style::default().fg(t.fg)
            };
            Line::from(Span::styled(m.clone(), style))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(log_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(t.border))
                .title(" log "),
        ),
        chunks[0],
    );

    let hint = "[j/k] nav  [Space] tag  [a] all  [m] model  [p] picker  [P] presets  [d] diff  [x] discard  [R] refetch  [s] save  [Esc] menu  [q] quit";
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(t.dim)),
        chunks[1],
    );
}
