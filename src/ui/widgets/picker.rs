//! Live-filter model picker (PRD FE-3.3). Renders catalog + input cursor.

use crate::ui::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let w = 70u16;
    let h = 16u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(3), // border + 1-line input
            ratatui::layout::Constraint::Min(1),
        ])
        .split(popup);

    frame.render_widget(Clear, popup);

    let input = format!(
        "> {}{}",
        app.modal_input,
        if app.modal_input.is_empty() {
            "start typing…"
        } else {
            ""
        }
    );
    let t = &app.theme;
    let loading = if app.fetch_pending > 0 {
        " (loading models...)"
    } else {
        ""
    };
    let focus_tag = match app.modal_focus {
        crate::ui::app::ModalFocus::Input => "INPUT — Tab: list",
        crate::ui::app::ModalFocus::List => "LIST — Tab: type",
    };
    frame.render_widget(
        Paragraph::new(input)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(t.accent))
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(format!(" filter [{focus_tag}]{loading} ")),
            )
            .wrap(Wrap { trim: false }),
        chunks[0],
    );

    let items: Vec<ListItem> = app
        .picker_items
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let marker = if i == app.picker_cursor { "> " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::raw(marker),
                Span::styled(
                    m.clone(),
                    if i == app.picker_cursor {
                        Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(t.fg)
                    },
                ),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if !app.picker_items.is_empty() {
        state.select(Some(app.picker_cursor));
    }
    frame.render_stateful_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(t.border))
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(" results "),
        ),
        chunks[1],
        &mut state,
    );
}
