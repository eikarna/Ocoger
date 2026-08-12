//! Two-band form: agent parameters (left) + global config (right).

use crate::ui::app::{App, Panel};
use crate::ui::widgets::util::fit;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

fn band_block(title: &str, active: bool, fg: ratatui::style::Color) -> Block<'static> {
    let mut b = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title.to_string());
    if active {
        b = b.border_style(Style::default().fg(fg));
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
        " Global config (opencode.jsonc ·global = read-only) ",
    );
}

fn render_band(frame: &mut Frame, area: Rect, app: &App, band: Panel, title: &str) {
    let active = app.form_band == band;
    let n = app.form_item_count_at(band);
    let mut items = Vec::with_capacity(n);
    for i in 0..n {
        let row = format_row_at(app, band, i, active);
        items.push(row);
    }

    let mut state = ListState::default();
    if active && n > 0 {
        state.select(Some(app.form_cursor));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(band_block(title, active, app.theme.accent))
            .highlight_style(Style::default()),
        area,
        &mut state,
    );
}

fn format_row_at(app: &App, band: Panel, idx: usize, band_is_active: bool) -> ListItem<'static> {
    let t = &app.theme;
    let label = app.form_label_at(band, idx);
    let mut val = app.form_value_at(band, idx);
    let highlighted = band_is_active && app.form_cursor == idx;

    // Read-only provenance marker on global-only keys.
    let is_readonly = band == Panel::GlobalConfig && label.ends_with("·global");
    let label_style = if is_readonly {
        Style::default()
            .fg(t.dim)
            .add_modifier(ratatui::style::Modifier::ITALIC)
    } else {
        Style::default().fg(t.dim)
    };
    if is_readonly {
        val = format!("{val} (ro)");
    }

    // Both form bands have a fixed width. Fit rather than spilling long model
    // ids into the next panel — this was visible as `darkdarkdark`/URLs crossing
    // the vertical border in the screenshot.
    let label = fit(&label, 22);
    let value_width = 28;
    ListItem::new(Line::from(vec![
        Span::styled(format!(" {label}"), label_style),
        Span::styled(
            fit(&val, value_width),
            if highlighted {
                Style::default().fg(t.accent).bg(t.highlight_bg)
            } else {
                Style::default().fg(t.fg)
            },
        ),
    ]))
    .style(if highlighted {
        Style::default().bg(t.highlight_bg)
    } else {
        Style::default()
    })
}

// Legacy helper retained for readability; calls into `format_row_at` via app.
#[allow(dead_code)]

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
