//! Pre-save diff view (PRD §9): unified line diff colored green/red.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: Rect, diff: Option<&str>) {
    let w = area.width.saturating_mul(90) / 100;
    let h = area.height.saturating_mul(85).div_ceil(100);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let text_lines: Vec<Line> = match diff {
        None => vec![Line::from("No staged changes to preview.")],
        Some(d) => d
            .lines()
            .map(|l| {
                let style = match l.chars().next() {
                    Some('+') => Style::default().fg(Color::Green),
                    Some('-') => Style::default().fg(Color::Red),
                    _ => Style::default(),
                };
                Line::from(Span::styled(l.to_string(), style))
            })
            .collect(),
    };

    frame.render_widget(
        Paragraph::new(text_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Diff preview "),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}
