//! Agent list pane: checkbox-style multi-select preview per PRD §4.

use crate::core::agent_parser::AgentFile;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect, agents: &[AgentFile], cursor: usize) {
    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let check = if a.is_selected { "[x]" } else { "[ ]" };
            let marker = if i == cursor { "> " } else { "  " };
            let style = if i == cursor {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::raw(marker),
                Span::raw(format!("{check} ")),
                Span::styled(
                    a.path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| a.path.display().to_string()),
                    style,
                ),
            ]))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" SUBAGENTS (.opencode/agents/*.md) ");

    if agents.is_empty() {
        f.render_widget(
            Paragraph::new("No agents found. Drop .md files into .opencode/agents/").block(block),
            area,
        );
        return;
    }

    let mut state = ListState::default();
    state.select(Some(cursor));
    f.render_stateful_widget(List::new(items).block(block), area, &mut state);
}
