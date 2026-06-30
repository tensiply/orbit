mod launch;
mod popup;
mod sessions;
mod system;

use crate::app::{App, Mode, Tab};
use ratatui::{
    Frame,
    layout::Constraint,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = ratatui::layout::Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    render_tab_bar(f, app, chunks[0]);

    match app.tab {
        Tab::Sessions => sessions::render(f, app, chunks[1]),
        Tab::Launch => launch::render(f, app, chunks[1]),
        Tab::System => system::render(f, app, chunks[1]),
    }

    render_footer(f, app, chunks[2]);

    // Popup overlays (rendered last so they appear on top)
    match &app.mode {
        Mode::Help => popup::render_help(f, area),
        Mode::ConfirmKill(s) => popup::render_confirm_kill(f, area, s.clone()),
        Mode::SessionDetails(s) => popup::render_details(f, area, s.clone()),
        Mode::AddMcp(state) => popup::render_add_mcp(f, area, state, &app.sys.default_tenant),
        Mode::ConfirmRemoveMcp(entry) => popup::render_confirm_remove_mcp(f, area, entry),
        Mode::FieldSelect(state) => popup::render_field_select(f, area, state),
        Mode::Normal => {}
    }
}

fn render_tab_bar(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let ws_name = app.active_workspace_name().to_string();
    let ws_hint = if app.workspaces.len() > 1 {
        Span::styled("[w]", Style::default().fg(Color::Cyan))
    } else {
        Span::styled("[w]", Style::default().fg(Color::DarkGray))
    };

    let line = Line::from(vec![
        Span::raw(" "),
        tab_span("[1]", app.tab == Tab::Sessions),
        Span::styled(" Sessions  ", tab_label_style(app.tab == Tab::Sessions)),
        tab_span("[2]", app.tab == Tab::Launch),
        Span::styled(" Launch  ", tab_label_style(app.tab == Tab::Launch)),
        tab_span("[3]", app.tab == Tab::System),
        Span::styled(" System  ", tab_label_style(app.tab == Tab::System)),
        Span::styled("─  ", Style::default().fg(Color::DarkGray)),
        ws_hint,
        Span::styled(format!(" {ws_name}"), Style::default().fg(Color::Yellow)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn tab_span(label: &'static str, active: bool) -> Span<'static> {
    if active {
        Span::styled(
            label,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(label, Style::default().fg(Color::DarkGray))
    }
}

fn tab_label_style(active: bool) -> Style {
    if active {
        Style::default()
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn render_footer(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let line = if let Some(msg) = &app.status_msg {
        Line::from(vec![
            Span::styled(" ! ", Style::default().fg(Color::Yellow)),
            Span::styled(msg.clone(), Style::default().fg(Color::Yellow)),
        ])
    } else {
        match app.tab {
            Tab::Sessions => Line::from(vec![
                hint(" [Tab]"),
                Span::raw(" switch  "),
                hint("[↑↓/jk]"),
                Span::raw(" nav  "),
                hint("[a/↵]"),
                Span::raw(" attach  "),
                hint("[K]"),
                Span::raw(" kill  "),
                hint("[d]"),
                Span::raw(" details  "),
                hint("[c]"),
                Span::raw(" clean  "),
                hint("[w]"),
                Span::raw(" workspace  "),
                hint("[?]"),
                Span::raw(" help  "),
                hint("[q]"),
                Span::raw(" quit"),
            ]),
            Tab::Launch => Line::from(vec![
                hint(" [Tab]"),
                Span::raw(" switch  "),
                hint("[↑↓]"),
                Span::raw(" fields  "),
                hint("[←→]"),
                Span::raw(" engine  "),
                hint("[Space]"),
                Span::raw(" toggle  "),
                hint("[↵]"),
                Span::raw(" confirm/launch  "),
                hint("[Esc]"),
                Span::raw(" back"),
            ]),
            Tab::System => Line::from(vec![
                hint(" [Tab]"),
                Span::raw(" switch  "),
                hint("[↑↓/jk]"),
                Span::raw(" MCP nav  "),
                hint("[a]"),
                Span::raw(" add  "),
                hint("[x]"),
                Span::raw(" remove  "),
                hint("[s]"),
                Span::raw(" daemon  "),
                hint("[w]"),
                Span::raw(" workspace  "),
                hint("[r]"),
                Span::raw(" refresh  "),
                hint("[q]"),
                Span::raw(" quit"),
            ]),
        }
    };
    f.render_widget(Paragraph::new(line), area);
}

fn hint(s: &'static str) -> Span<'static> {
    Span::styled(s, Style::default().fg(Color::Cyan))
}
