use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Span,
    widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table},
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let dim = app.palette.dim;
    let chunks =
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(area);

    let peers = &app.peers_state.peers;
    let peer_rows: Vec<Row> = peers
        .iter()
        .map(|p| {
            Row::new(vec![
                Cell::from(p.addr.as_str()),
                Cell::from(format!("{:?}", p.role)),
                Cell::from(format!("{} req", p.requests)),
            ])
        })
        .collect();

    let peers_table = Table::new(
        peer_rows,
        [
            Constraint::Percentage(50),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ],
    )
    .header(Row::new(vec!["ADDRESS", "ROLE", "REQUESTS"]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(dim))
            .title(Span::styled(
                " Connected Peers — [S] share  [r] refresh ",
                Style::default().fg(dim),
            ))
            .padding(Padding::uniform(1)),
    );

    f.render_widget(peers_table, chunks[0]);

    let info_text = if app.serving_active {
        "Sharing active. Press [S] to manage sharing."
    } else {
        "Not sharing. Press [S] to start sharing via LAN."
    };

    let info = Paragraph::new(info_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(dim))
            .title(Span::styled(
                " LAN Status ",
                Style::default().fg(dim),
            ))
            .padding(Padding::uniform(1)),
    );
    f.render_widget(info, chunks[1]);
}
