use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};

use super::{App, Mode};
use crate::color::ColorMode;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(vertical[0]);

    draw_tree(frame, app, main[0]);
    draw_detail(frame, app, main[1]);
    draw_status(frame, app, vertical[1]);
}

fn draw_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let height = area.height.saturating_sub(2) as usize;
    if app.cursor >= app.visible.len() {
        app.cursor = app.visible.len().saturating_sub(1);
    }
    if height > 0 {
        if app.cursor < app.tree_offset {
            app.tree_offset = app.cursor;
        }
        if app.cursor >= app.tree_offset + height {
            app.tree_offset = app.cursor - height + 1;
        }
    }

    let mut items = Vec::new();
    for (position, row_index) in app
        .visible
        .iter()
        .skip(app.tree_offset)
        .take(height)
        .enumerate()
    {
        let row = &app.model.rows[*row_index];
        let open = app.model.is_expanded(*row_index) || !app.model.filter.is_empty();
        let marker = if !row.expandable {
            "  "
        } else if open {
            "▾ "
        } else {
            "▸ "
        };
        let selected = app.tree_offset + position == app.cursor;
        let mut style = Style::default();
        if selected {
            style = style.add_modifier(Modifier::REVERSED);
        }
        let prefix = format!("{}{}", "  ".repeat(row.depth), marker);
        let colored = !matches!(app.color, ColorMode::Never);
        let line = match (colored, row.label.split_once(" -[")) {
            (true, Some((head, tail))) => Line::from(vec![
                Span::raw(format!("{prefix}{head}")),
                Span::styled(
                    format!(" -[{tail}"),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]),
            _ => Line::from(format!("{prefix}{}", row.label)),
        };
        items.push(ListItem::new(line).style(style));
    }

    let list = List::new(items).block(Block::bordered().title(top_level_label(app)));
    frame.render_widget(list, area);
}

fn top_level_label(app: &App) -> String {
    let Some(row_index) = app.visible.get(app.cursor).copied() else {
        return String::from("devices");
    };
    let mut index = row_index;
    while let Some(parent) = app.model.rows[index].parent {
        index = parent;
    }
    app.model.rows[index].label.clone()
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let title = app
        .visible
        .get(app.cursor)
        .and_then(|row_index| app.model.rows[*row_index].address)
        .map(|address| {
            format!(
                "{:04x}:{:02x}:{:02x}.{}",
                address.domain, address.bus, address.slot, address.function
            )
        })
        .unwrap_or_else(|| String::from("detail"));
    let paragraph = Paragraph::new(app.detail.clone())
        .scroll((app.detail_scroll, 0))
        .block(Block::bordered().title(title));
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let total = app
        .model
        .rows
        .iter()
        .filter(|row| row.address.is_some())
        .count();
    let shown = if app.model.filter.is_empty() {
        total
    } else {
        let lower = app.model.filter.to_lowercase();
        app.model
            .rows
            .iter()
            .filter(|row| row.address.is_some() && row.search_text.to_lowercase().contains(&lower))
            .count()
    };
    let text = match app.mode {
        Mode::Normal => {
            let mut text = format!(
                " j/k move  l/h expand/collapse  / filter  PgUp/PgDn scroll  q quit  devices: {shown}/{total}"
            );
            if !app.model.filter.is_empty() {
                text.push_str(&format!("  filter: {}", app.model.filter));
            }
            text
        }
        Mode::Filter => format!(" filter: {}█  Enter apply  Esc clear", app.filter_input),
    };
    frame.render_widget(Paragraph::new(text), area);
}
