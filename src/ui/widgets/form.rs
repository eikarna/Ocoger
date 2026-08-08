//! Two-band form: agent parameters (left) + global config (right).

use crate::ui::app::{App, Panel};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

fn band_block(title: &str, active: bool) -> Block<'static> {
    let mut b = Block::default()
        .borders(Borders::ALL)
        .title(title.to_string());
    if active {
        b = b.border_style(Style::default().fg(Color::Yellow));
    }
    b
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_band(frame, chunks[0], app, Panel::AgentParams, " Agent params ");
    render_band(
        frame,
        chunks[1],
        app,
        Panel::GlobalConfig,
        " Global config (opencode.jsonc) ",
    );
}

fn render_band(frame: &mut Frame, area: Rect, app: &App, band: Panel, title: &str) {
    let active = app.form_band == band;
    let n = app.form_item_count_at(band);
    let mut items = Vec::with_capacity(n);
    for i in 0..n {
        let row = format_row(
            app.form_label_at(band, i),
            app.form_value_at(band, i),
            active && app.form_cursor == i,
        );
        items.push(row);
    }

    let mut state = ListState::default();
    if active && n > 0 {
        state.select(Some(app.form_cursor));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(band_block(title, active))
            .highlight_style(Style::default()),
        area,
        &mut state,
    );
}

fn format_row(label: String, value: String, highlighted: bool) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(format!(" {label:22}"), Style::default().fg(Color::Gray)),
        Span::styled(
            value,
            if highlighted {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            },
        ),
    ]))
    .style(if highlighted {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    })
}

impl App {
    pub fn form_item_count_at(&self, band: Panel) -> usize {
        match band {
            Panel::AgentParams => 5,
            Panel::GlobalConfig => self.config_items.len(),
        }
    }

    pub fn form_label_at(&self, band: Panel, idx: usize) -> String {
        match band {
            Panel::AgentParams => {
                const LABELS: [&str; 5] =
                    ["model", "temperature", "top_k", "top_p", "reasoning_effort"];
                LABELS.get(idx).copied().unwrap_or("?").to_string()
            }
            Panel::GlobalConfig => self
                .config_items
                .get(idx)
                .map(|i| i.label.clone())
                .unwrap_or_default(),
        }
    }

    pub fn form_value_at(&self, band: Panel, idx: usize) -> String {
        match band {
            Panel::AgentParams => self
                .agents
                .get(self.cursor)
                .map(|a| {
                    let fm = &a.frontmatter;
                    match idx {
                        0 => fm.model.clone(),
                        1 => fm
                            .temperature
                            .map(|v| format!("{v:.2}"))
                            .unwrap_or_else(|| "-".into()),
                        2 => fm
                            .top_k
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".into()),
                        3 => fm
                            .top_p
                            .map(|v| format!("{v:.2}"))
                            .unwrap_or_else(|| "-".into()),
                        4 => fm.reasoning_effort.clone().unwrap_or_else(|| "-".into()),
                        _ => "?".into(),
                    }
                })
                .unwrap_or_default(),
            Panel::GlobalConfig => self
                .config_items
                .get(idx)
                .map(|i| i.value.clone())
                .unwrap_or_default(),
        }
    }
}
