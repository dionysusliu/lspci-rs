use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

/// Dim field labels (before ': ') and colorize unavailable values red.
/// Called before text_from_ansi to add TUI-specific coloring on top of the
/// CLI's existing ANSI output.
pub fn colorize_detail(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let Some(pos) = line.find(": ") else {
                return line.to_owned();
            };
            let (label, rest) = line.split_at(pos);
            let value = &rest[2..];
            if value.contains("<unavailable:") {
                format!("\x1b[2m{label}\x1b[0m: \x1b[31m{value}\x1b[0m")
            } else {
                format!("\x1b[2m{label}\x1b[0m: {value}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert ANSI-colored text into an owned ratatui Text.
/// Only SGR sequences are interpreted; unknown codes are ignored.
pub fn text_from_ansi(input: &str) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buffer = String::new();
    let mut style = Style::default();

    let mut rest = input;
    loop {
        let next = rest.find(|c| c == '\x1b' || c == '\n');
        let Some(pos) = next else {
            buffer.push_str(rest);
            break;
        };
        buffer.push_str(&rest[..pos]);
        if rest.as_bytes()[pos] == b'\n' {
            push_span(&mut spans, &mut buffer, style);
            lines.push(Line::from(std::mem::take(&mut spans)));
            rest = &rest[pos + 1..];
            continue;
        }
        let after = &rest[pos + 1..];
        if let Some(stripped) = after.strip_prefix('[') {
            if let Some(end) = stripped.find(|c: char| ('@'..='~').contains(&c)) {
                if stripped.as_bytes()[end] == b'm' {
                    push_span(&mut spans, &mut buffer, style);
                    style = apply_sgr(style, &stripped[..end]);
                }
                rest = &stripped[end + 1..];
                continue;
            }
        }
        buffer.push('\x1b');
        rest = after;
    }

    push_span(&mut spans, &mut buffer, style);
    lines.push(Line::from(spans));
    Text::from(lines)
}

fn push_span(spans: &mut Vec<Span<'static>>, buffer: &mut String, style: Style) {
    if !buffer.is_empty() {
        spans.push(Span::styled(std::mem::take(buffer), style));
    }
}

fn apply_sgr(style: Style, params: &str) -> Style {
    match params {
        "0" | "" => Style::default(),
        "1;36" => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "2" => style.add_modifier(Modifier::DIM),
        "31" => Style::default().fg(Color::Red),
        "32" => Style::default().fg(Color::Green),
        _ => style,
    }
}
