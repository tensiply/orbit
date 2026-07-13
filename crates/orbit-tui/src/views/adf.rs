/// Atlassian Document Format → ratatui Lines renderer.
///
/// Handles: paragraph, heading, text (bold/italic/code/strike/link),
/// bulletList, orderedList, codeBlock, blockquote, table, mediaSingle,
/// hardBreak, rule, mention, emoji.
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use serde_json::Value;

pub fn render(val: &Value, dim: Color) -> Vec<Line<'static>> {
    let empty = vec![];
    let nodes = val
        .get("content")
        .and_then(|c| c.as_array())
        .unwrap_or(&empty);
    render_blocks(nodes, 0, &mut 0, dim)
}

// ── block nodes ───────────────────────────────────────────────────────────────

fn render_blocks(
    nodes: &[Value],
    list_depth: usize,
    list_idx: &mut usize,
    dim: Color,
) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for node in nodes {
        out.extend(render_block(node, list_depth, list_idx, dim));
    }
    out
}

fn render_block(
    node: &Value,
    list_depth: usize,
    list_idx: &mut usize,
    dim: Color,
) -> Vec<Line<'static>> {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let content = node.get("content").and_then(|c| c.as_array());

    match node_type {
        "doc" => {
            let empty = vec![];
            render_blocks(content.unwrap_or(&empty), 0, &mut 0, dim)
        }

        "paragraph" => {
            let spans = collect_inline(content.map_or(&[], |v| v), dim);
            let mut out = vec![Line::from(spans)];
            out.push(Line::from(""));
            out
        }

        "heading" => {
            let level = node
                .get("attrs")
                .and_then(|a| a.get("level"))
                .and_then(|l| l.as_u64())
                .unwrap_or(1);
            let style = heading_style(level);
            let spans: Vec<Span<'static>> = collect_inline(content.map_or(&[], |v| v), dim)
                .into_iter()
                .map(|s| Span::styled(s.content.to_string(), s.style.patch(style)))
                .collect();
            let mut out = vec![Line::from(spans)];
            out.push(Line::from(""));
            out
        }

        "bulletList" => {
            let empty = vec![];
            let items = content.unwrap_or(&empty);
            let mut out = Vec::new();
            for item in items {
                let prefix = format!("{}• ", "  ".repeat(list_depth));
                let item_content = item.get("content").and_then(|c| c.as_array());
                let inner =
                    render_blocks(item_content.map_or(&[], |v| v), list_depth + 1, &mut 0, dim);
                for (i, line) in inner.into_iter().enumerate() {
                    if i == 0 {
                        let mut spans =
                            vec![Span::styled(prefix.clone(), Style::default().fg(dim))];
                        spans.extend(line.spans);
                        out.push(Line::from(spans));
                    } else {
                        out.push(line);
                    }
                }
            }
            out.push(Line::from(""));
            out
        }

        "orderedList" => {
            let empty = vec![];
            let items = content.unwrap_or(&empty);
            let start = node
                .get("attrs")
                .and_then(|a| a.get("order"))
                .and_then(|o| o.as_u64())
                .unwrap_or(1) as usize;
            let mut out = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let n = start + i;
                let prefix = format!("{}{}. ", "  ".repeat(list_depth), n);
                let item_content = item.get("content").and_then(|c| c.as_array());
                let inner =
                    render_blocks(item_content.map_or(&[], |v| v), list_depth + 1, &mut 0, dim);
                for (j, line) in inner.into_iter().enumerate() {
                    if j == 0 {
                        let mut spans =
                            vec![Span::styled(prefix.clone(), Style::default().fg(dim))];
                        spans.extend(line.spans);
                        out.push(Line::from(spans));
                    } else {
                        out.push(line);
                    }
                }
            }
            out.push(Line::from(""));
            out
        }

        "codeBlock" => {
            let lang = node
                .get("attrs")
                .and_then(|a| a.get("language"))
                .and_then(|l| l.as_str())
                .unwrap_or("");
            let mut out = Vec::new();
            if !lang.is_empty() {
                out.push(Line::from(Span::styled(
                    format!("  ┌─ {lang} "),
                    Style::default().fg(dim),
                )));
            } else {
                out.push(Line::from(Span::styled(
                    "  ┌──────",
                    Style::default().fg(dim),
                )));
            }
            for span in collect_inline(content.map_or(&[], |v| v), dim) {
                out.push(Line::from(vec![
                    Span::styled("  │ ", Style::default().fg(dim)),
                    Span::styled(span.content.to_string(), Style::default().fg(Color::Cyan)),
                ]));
            }
            out.push(Line::from(Span::styled(
                "  └──────",
                Style::default().fg(dim),
            )));
            out.push(Line::from(""));
            out
        }

        "blockquote" => {
            let empty = vec![];
            let inner = render_blocks(content.unwrap_or(&empty), list_depth, &mut 0, dim);
            let mut out = Vec::new();
            for line in inner {
                let mut spans = vec![Span::styled("  │ ", Style::default().fg(dim))];
                spans.extend(
                    line.spans
                        .into_iter()
                        .map(|s| Span::styled(s.content.to_string(), s.style.fg(dim))),
                );
                out.push(Line::from(spans));
            }
            out.push(Line::from(""));
            out
        }

        "table" => {
            let mut out = render_table(node, dim);
            out.push(Line::from(""));
            out
        }

        "mediaSingle" | "mediaGroup" => {
            let mut out = vec![Line::from(Span::styled(
                "  [🖼  Attachment]",
                Style::default().fg(dim).add_modifier(Modifier::DIM),
            ))];
            out.push(Line::from(""));
            out
        }

        "rule" => {
            vec![
                Line::from(Span::styled("─".repeat(60), Style::default().fg(dim))),
                Line::from(""),
            ]
        }

        // pass-through containers
        "listItem" => render_blocks(content.map_or(&[], |v| v), list_depth, list_idx, dim),

        _ => vec![],
    }
}

// ── inline nodes ──────────────────────────────────────────────────────────────

fn collect_inline(nodes: &[Value], dim: Color) -> Vec<Span<'static>> {
    nodes.iter().flat_map(|n| inline_span(n, dim)).collect()
}

fn inline_span(node: &Value, dim: Color) -> Vec<Span<'static>> {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match node_type {
        "text" => {
            let text = node
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let style = marks_style(node.get("marks").and_then(|m| m.as_array()));
            vec![Span::styled(text, style)]
        }

        "hardBreak" => vec![Span::raw("\n")],

        "mention" => {
            let name = node
                .get("attrs")
                .and_then(|a| a.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("@?")
                .to_string();
            vec![Span::styled(name, Style::default().fg(Color::Cyan))]
        }

        "emoji" => {
            let text = node
                .get("attrs")
                .and_then(|a| a.get("text"))
                .or_else(|| node.get("attrs").and_then(|a| a.get("shortName")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            vec![Span::raw(text)]
        }

        "inlineCard" | "blockCard" => {
            let url = node
                .get("attrs")
                .and_then(|a| a.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("link")
                .to_string();
            vec![Span::styled(
                format!("[🔗 {url}]"),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::UNDERLINED),
            )]
        }

        "media" | "file" => vec![Span::styled(
            "[📎 file]".to_string(),
            Style::default().fg(dim),
        )],

        _ => {
            // Unknown inline: recurse into content if available
            if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
                collect_inline(content, dim)
            } else {
                vec![]
            }
        }
    }
}

fn marks_style(marks: Option<&Vec<Value>>) -> Style {
    let mut style = Style::default();
    let Some(marks) = marks else { return style };
    for mark in marks {
        match mark.get("type").and_then(|t| t.as_str()) {
            Some("strong") => style = style.add_modifier(Modifier::BOLD),
            Some("em") => style = style.add_modifier(Modifier::ITALIC),
            Some("code") => style = style.fg(Color::Cyan),
            Some("strike") => style = style.add_modifier(Modifier::CROSSED_OUT),
            Some("underline") => style = style.add_modifier(Modifier::UNDERLINED),
            Some("link") => style = style.fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
            Some("textColor") => {
                if let Some(hex) = mark
                    .get("attrs")
                    .and_then(|a| a.get("color"))
                    .and_then(|c| c.as_str())
                {
                    style = style.fg(hex_to_color(hex));
                }
            }
            _ => {}
        }
    }
    style
}

fn hex_to_color(hex: &str) -> Color {
    let h = hex.trim_start_matches('#').to_lowercase();
    match h.as_str() {
        "ff0000" | "de350b" | "bf2600" => Color::Red,
        "ff8b00" | "ff991c" => Color::Yellow,
        "36b37e" | "00875a" | "006644" => Color::Green,
        "0052cc" | "0065ff" | "0747a6" => Color::Blue,
        "6554c0" | "403294" => Color::Magenta,
        "00b8d9" | "00a3bf" | "008da6" => Color::Cyan,
        _ => Color::White,
    }
}

fn heading_style(level: u64) -> Style {
    match level {
        1 => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        2 => Style::default()
            .fg(Color::Reset)
            .add_modifier(Modifier::BOLD),
        3 => Style::default()
            .fg(Color::Reset)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().add_modifier(Modifier::BOLD),
    }
}

// ── table renderer ────────────────────────────────────────────────────────────

fn render_table(node: &Value, dim: Color) -> Vec<Line<'static>> {
    let empty = vec![];
    let rows = node
        .get("content")
        .and_then(|c| c.as_array())
        .unwrap_or(&empty);

    let table_data: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let row_content = row
                .get("content")
                .and_then(|c| c.as_array())
                .unwrap_or(&empty);
            row_content.iter().map(extract_cell_text).collect()
        })
        .collect();

    if table_data.is_empty() {
        return vec![];
    }

    let col_count = table_data.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return vec![];
    }

    let col_widths: Vec<usize> = (0..col_count)
        .map(|c| {
            table_data
                .iter()
                .map(|row| row.get(c).map(|s| s.chars().count()).unwrap_or(0))
                .max()
                .unwrap_or(0)
                .max(1)
        })
        .collect();

    let mut out = Vec::new();

    for (row_idx, row) in table_data.iter().enumerate() {
        if row_idx == 0 {
            out.push(table_border_line(&col_widths, '┌', '┬', '┐', '─', dim));
        } else if row_idx == 1 {
            let first_row = rows
                .first()
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array());
            let first_is_header = first_row
                .and_then(|cells| cells.first())
                .and_then(|c| c.get("type"))
                .and_then(|t| t.as_str())
                == Some("tableHeader");
            if first_is_header {
                out.push(table_border_line(&col_widths, '╞', '╪', '╡', '═', dim));
            } else {
                out.push(table_border_line(&col_widths, '├', '┼', '┤', '─', dim));
            }
        } else {
            out.push(table_border_line(&col_widths, '├', '┼', '┤', '─', dim));
        }

        let row_node = rows.get(row_idx);
        let first_cell_type = row_node
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|cells| cells.first())
            .and_then(|c| c.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let is_header_row = first_cell_type == "tableHeader";

        let mut spans = vec![Span::styled("│", Style::default().fg(dim))];
        for (col_idx, width) in col_widths.iter().enumerate() {
            let cell_text = row.get(col_idx).cloned().unwrap_or_default();
            let padded = format!(" {:<width$} ", cell_text, width = width);
            let style = if is_header_row {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            spans.push(Span::styled(padded, style));
            spans.push(Span::styled("│", Style::default().fg(dim)));
        }
        out.push(Line::from(spans));
    }

    out.push(table_border_line(&col_widths, '└', '┴', '┘', '─', dim));
    out
}

fn table_border_line(
    widths: &[usize],
    left: char,
    mid: char,
    right: char,
    fill: char,
    dim: Color,
) -> Line<'static> {
    let mut s = left.to_string();
    for (i, w) in widths.iter().enumerate() {
        s.push_str(&fill.to_string().repeat(w + 2));
        if i + 1 < widths.len() {
            s.push(mid);
        }
    }
    s.push(right);
    Line::from(Span::styled(s, Style::default().fg(dim)))
}

fn extract_cell_text(cell: &Value) -> String {
    let empty = vec![];
    let content = cell
        .get("content")
        .and_then(|c| c.as_array())
        .unwrap_or(&empty);
    extract_nodes_text(content)
}

fn extract_nodes_text(nodes: &[Value]) -> String {
    nodes
        .iter()
        .map(extract_node_text)
        .collect::<Vec<_>>()
        .join("")
}

fn extract_node_text(node: &Value) -> String {
    if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
        return text.to_string();
    }
    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        return extract_nodes_text(content);
    }
    String::new()
}
