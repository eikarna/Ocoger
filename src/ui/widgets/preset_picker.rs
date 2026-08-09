//! Preset picker modal (Phase 4).
//!
//! Three sub-states:
//!   - `Mode::Preset`         — live-filtered list, cursor, footer with keys
//!   - `Mode::PresetNameNew`  — name prompt before capturing selection
//!   - `Mode::PresetConfirmAll` — yes/no guard before apply-to-all

use crate::ui::app::{App, Mode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let w = 78u16;
    let h = match app.mode {
        Mode::PresetConfirmAll => 7,
        Mode::PresetNameNew => 5,
        _ => 18,
    };
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    frame.render_widget(Clear, popup);

    match app.mode {
        Mode::PresetNameNew => {
            let text = format!(
                "save selected-agents' settings as preset\n> {}",
                app.modal_input
            );
            frame.render_widget(
                Paragraph::new(text)
                    .block(Block::default().borders(Borders::ALL).title(" new preset "))
                    .wrap(Wrap { trim: false }),
                popup,
            );
        }
        Mode::PresetConfirmAll => {
            let name = app
                .pending_preset
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let text = format!(
                "apply preset '{name}' to ALL agents?\nthis overwrites model/temp/top_k/top_p/effort on every agent.\n\n[y] confirm    [n/Esc] cancel"
            );
            frame.render_widget(
                Paragraph::new(text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" apply to all? "),
                    )
                    .wrap(Wrap { trim: false }),
                popup,
            );
        }
        _ => {
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(2),
                    ratatui::layout::Constraint::Min(1),
                    ratatui::layout::Constraint::Length(1),
                ])
                .split(popup);

            let input = format!(
                "> {}{}",
                app.modal_input,
                if app.modal_input.is_empty() {
                    "type to filter…"
                } else {
                    ""
                }
            );
            frame.render_widget(
                Paragraph::new(input).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" preset filter "),
                ),
                chunks[0],
            );

            let items: Vec<ListItem> = app
                .preset_items
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let marker = if i == app.preset_cursor { "> " } else { "  " };
                    let desc = p.description.as_deref().unwrap_or("");
                    let label = format!("{:18} {:32} {:24}", p.name, desc, p.model);
                    ListItem::new(Line::from(vec![
                        Span::raw(marker),
                        Span::styled(
                            label,
                            if i == app.preset_cursor {
                                Style::default().fg(Color::Yellow)
                            } else {
                                Style::default()
                            },
                        ),
                    ]))
                })
                .collect();

            let mut state = ListState::default();
            if !app.preset_items.is_empty() {
                state.select(Some(app.preset_cursor));
            }
            frame.render_stateful_widget(
                List::new(items).block(Block::default().borders(Borders::ALL).title(" presets ")),
                chunks[1],
                &mut state,
            );

            let footer =
                "Enter=apply selected · n=new-from-selection · d=del · Shift+A=apply to all · Esc=close";
            frame.render_widget(Paragraph::new(footer), chunks[2]);
        }
    }
}
