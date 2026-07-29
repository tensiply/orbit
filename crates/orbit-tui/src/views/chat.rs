use orbit_core::{
    ipc::PlanStreamEvent,
    plan::{NodeStatus, NodeSummary},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::widget::TextInput;

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChatMode {
    Input,
    Scroll,
}

#[derive(Clone)]
#[allow(dead_code)] // ScopeDetected will be emitted when scope detection is wired
pub enum OrbitContent {
    Text(String),
    ScopeDetected { path: String, confidence: String },
    PlanInline { plan_id: String, nodes: Vec<NodeSummary>, done: bool, failed: bool },
    ApprovalNeeded { plan_id: String, node_id: String, label: String },
    Error(String),
}

#[derive(Clone)]
pub enum ChatMessage {
    User { text: String },
    System { text: String },
    Orbit { content: OrbitContent },
}

#[allow(dead_code)]
pub struct ActivePlan {
    pub plan_id: String,
    pub msg_idx: usize,
    pub rx: tokio::sync::mpsc::Receiver<PlanStreamEvent>,
}

pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub scroll: u16,
    pub input: TextInput,
    pub mode: ChatMode,
    pub active_plan: Option<ActivePlan>,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: vec![ChatMessage::System {
                text: "orbit — type a goal and press Enter, or /help for commands".into(),
            }],
            scroll: 0,
            input: TextInput::new("chat", "Describe a goal or /help…"),
            mode: ChatMode::Input,
            active_plan: None,
        }
    }

    pub fn push_user(&mut self, text: String) {
        self.messages.push(ChatMessage::User { text });
        self.scroll = u16::MAX;
    }

    pub fn push_orbit(&mut self, content: OrbitContent) {
        self.messages.push(ChatMessage::Orbit { content });
        self.scroll = u16::MAX;
    }

    pub fn push_plan_inline(&mut self, plan_id: String, nodes: Vec<NodeSummary>) -> usize {
        let idx = self.messages.len();
        self.messages.push(ChatMessage::Orbit {
            content: OrbitContent::PlanInline { plan_id, nodes, done: false, failed: false },
        });
        self.scroll = u16::MAX;
        idx
    }

    pub fn update_node_status(&mut self, msg_idx: usize, node_id: &str, status: NodeStatus) {
        if let Some(ChatMessage::Orbit {
            content: OrbitContent::PlanInline { nodes, .. },
        }) = self.messages.get_mut(msg_idx)
            && let Some(n) = nodes.iter_mut().find(|n| n.id == node_id)
        {
            n.status = status;
        }
    }

    pub fn mark_plan_done(&mut self, msg_idx: usize, failed: bool) {
        if let Some(ChatMessage::Orbit {
            content: OrbitContent::PlanInline { done, failed: f, .. },
        }) = self.messages.get_mut(msg_idx)
        {
            *done = !failed;
            *f = failed;
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, max_scroll: u16) {
        if self.scroll < max_scroll {
            self.scroll += 1;
        }
    }

    /// Drain the stream channel and apply updates. Returns true if anything changed.
    pub fn drain_stream(&mut self) -> bool {
        let Some(ref mut ap) = self.active_plan else {
            return false;
        };

        let mut events = Vec::new();
        let mut disconnected = false;
        loop {
            match ap.rx.try_recv() {
                Ok(evt) => events.push(evt),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        let changed = !events.is_empty() || disconnected;
        if changed {
            let msg_idx = self.active_plan.as_ref().map_or(0, |ap| ap.msg_idx);
            let plan_id = self
                .active_plan
                .as_ref()
                .map(|ap| ap.plan_id.clone())
                .unwrap_or_default();

            let mut terminal_seen = false;
            for evt in events {
                match evt {
                    PlanStreamEvent::NodeStarted { node_id, .. } => {
                        self.update_node_status(msg_idx, &node_id, NodeStatus::Running);
                    }
                    PlanStreamEvent::NodeCompleted { node_id, .. } => {
                        self.update_node_status(msg_idx, &node_id, NodeStatus::Completed);
                    }
                    PlanStreamEvent::NodeFailed { node_id, error, .. } => {
                        self.update_node_status(msg_idx, &node_id, NodeStatus::Failed);
                        self.push_orbit(OrbitContent::Error(format!("{node_id}: {error}")));
                    }
                    PlanStreamEvent::NodeAwaitingApproval { node_id, label, .. } => {
                        self.update_node_status(msg_idx, &node_id, NodeStatus::AwaitingApproval);
                        self.push_orbit(OrbitContent::ApprovalNeeded {
                            plan_id: plan_id.clone(),
                            node_id,
                            label,
                        });
                    }
                    PlanStreamEvent::PlanCompleted { .. } => {
                        self.mark_plan_done(msg_idx, false);
                        terminal_seen = true;
                    }
                    PlanStreamEvent::PlanFailed { .. } => {
                        self.mark_plan_done(msg_idx, true);
                        self.push_orbit(OrbitContent::Error("Plan failed.".into()));
                        terminal_seen = true;
                    }
                    PlanStreamEvent::PlanReplanning { child_plan_id, .. } => {
                        self.push_orbit(OrbitContent::Text(format!("Replanning → {child_plan_id}")));
                    }
                    PlanStreamEvent::NodeOutput { .. } => {}
                }
            }

            if terminal_seen || disconnected {
                self.active_plan = None;
            }
        }

        changed
    }
}

// ── slash commands ────────────────────────────────────────────────────────────

pub enum SlashCommand {
    Help,
    Status,
    DryRun(String),
    Approve { plan_id: Option<String>, node_id: String },
    Cancel(Option<String>),
}

pub fn parse_slash(input: &str) -> Option<SlashCommand> {
    let s = input.trim();
    if s == "/help" {
        return Some(SlashCommand::Help);
    }
    if s == "/status" {
        return Some(SlashCommand::Status);
    }
    if let Some(rest) = s.strip_prefix("/dry-run ") {
        return Some(SlashCommand::DryRun(rest.trim().to_string()));
    }
    if let Some(rest) = s.strip_prefix("/approve ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        return match parts.as_slice() {
            [node_id] => Some(SlashCommand::Approve { plan_id: None, node_id: node_id.to_string() }),
            [plan_id, node_id] => Some(SlashCommand::Approve {
                plan_id: Some(plan_id.to_string()),
                node_id: node_id.to_string(),
            }),
            _ => None,
        };
    }
    if let Some(rest) = s.strip_prefix("/cancel") {
        let id = rest.trim();
        return Some(SlashCommand::Cancel(if id.is_empty() { None } else { Some(id.to_string()) }));
    }
    None
}

fn help_text() -> String {
    "  /dry-run <goal>   generate plan without executing\n  \
     /approve <node>   approve a node awaiting approval\n  \
     /cancel [id]      cancel the active plan\n  \
     /status           list recent plans\n  \
     Esc               scroll mode (j/k navigation)\n  \
     i                 return to input mode"
        .into()
}

// ── async action type ─────────────────────────────────────────────────────────

pub enum ChatAsyncAction {
    CreatePlan { goal: String, dry_run: bool },
    ApprovePlanNode { plan_id: String, node_id: String },
    CancelPlan(Option<String>),
    ListPlans,
}

/// Takes text from the input and clears it. Returns `None` if empty.
pub fn take_input(state: &mut ChatState) -> Option<String> {
    let text = state.input.value.trim().to_string();
    if text.is_empty() {
        return None;
    }
    state.input.value.clear();
    state.input.cursor = 0;
    Some(text)
}

/// Processes a submitted line. Returns an async action if needed.
pub fn handle_submit(state: &mut ChatState, text: String) -> Option<ChatAsyncAction> {
    state.push_user(text.clone());

    if let Some(cmd) = parse_slash(&text) {
        match cmd {
            SlashCommand::Help => {
                state.push_orbit(OrbitContent::Text(help_text()));
                None
            }
            SlashCommand::Status => Some(ChatAsyncAction::ListPlans),
            SlashCommand::DryRun(goal) => {
                state.push_orbit(OrbitContent::Text("Generating plan (dry run)…".into()));
                Some(ChatAsyncAction::CreatePlan { goal, dry_run: true })
            }
            SlashCommand::Approve { plan_id, node_id } => {
                let pid = plan_id
                    .or_else(|| state.active_plan.as_ref().map(|ap| ap.plan_id.clone()));
                if let Some(pid) = pid {
                    Some(ChatAsyncAction::ApprovePlanNode { plan_id: pid, node_id })
                } else {
                    state.push_orbit(OrbitContent::Error(
                        "No active plan — use: /approve <plan_id> <node_id>".into(),
                    ));
                    None
                }
            }
            SlashCommand::Cancel(id) => {
                let id = id.or_else(|| state.active_plan.as_ref().map(|ap| ap.plan_id.clone()));
                Some(ChatAsyncAction::CancelPlan(id))
            }
        }
    } else {
        state.push_orbit(OrbitContent::Text("Detecting scope…".into()));
        Some(ChatAsyncAction::CreatePlan { goal: text, dry_run: false })
    }
}

/// Applies a successful CreatePlan response.
pub fn apply_plan_created(
    state: &mut ChatState,
    id: String,
    nodes: Vec<NodeSummary>,
    dry_run: bool,
    stream_rx: Option<tokio::sync::mpsc::Receiver<PlanStreamEvent>>,
) {
    let is_placeholder = matches!(
        state.messages.last(),
        Some(ChatMessage::Orbit { content: OrbitContent::Text(t) })
        if t.contains("Detecting scope") || t.contains("Generating plan")
    );
    if is_placeholder {
        state.messages.pop();
    }
    let msg_idx = state.push_plan_inline(id.clone(), nodes);
    if dry_run {
        state.mark_plan_done(msg_idx, false);
        return;
    }
    if let Some(rx) = stream_rx {
        state.active_plan = Some(ActivePlan { plan_id: id, msg_idx, rx });
    }
}

/// Applies an error response.
pub fn apply_plan_error(state: &mut ChatState, msg: String) {
    let is_placeholder = matches!(
        state.messages.last(),
        Some(ChatMessage::Orbit { content: OrbitContent::Text(t) })
        if t.contains("Detecting scope") || t.contains("Generating plan")
    );
    if is_placeholder {
        state.messages.pop();
    }
    state.push_orbit(OrbitContent::Error(msg));
}

/// Formats a plan list as readable text.
pub fn format_plan_list(plans: &[orbit_core::plan::Plan]) -> String {
    if plans.is_empty() {
        return "No recent plans.".into();
    }
    plans
        .iter()
        .take(10)
        .map(|p| format!("  {} — {:?} ({} nodes)", p.id, p.status, p.nodes.len()))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── layout constants ─────────────────────────────────────────────────────────

/// Horizontal padding from the panel edge, applied to both history and input.
const H_PAD: u16 = 3;
/// Top padding inside the chat history (breathing room before the first message).
const V_PAD: u16 = 1;

// ── render ────────────────────────────────────────────────────────────────────

pub fn render(app: &mut App, f: &mut Frame, area: Rect) {
    let p = app.palette.clone();

    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(area);

    let history_area = chunks[0];
    let input_area = chunks[1];

    // Apply uniform H_PAD + V_PAD to history
    let inner_history = Rect {
        x: history_area.x + H_PAD,
        y: history_area.y + V_PAD,
        width: history_area.width.saturating_sub(H_PAD * 2),
        height: history_area.height.saturating_sub(V_PAD),
    };
    let box_width = inner_history.width as usize;

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    for msg in &app.chat.messages {
        render_message(msg, box_width, &p, &mut all_lines);
    }

    let total_lines = all_lines.len() as u16;
    let max_scroll = total_lines.saturating_sub(inner_history.height);
    if app.chat.scroll > max_scroll {
        app.chat.scroll = max_scroll;
    }

    f.render_widget(
        Paragraph::new(all_lines).scroll((app.chat.scroll, 0)),
        inner_history,
    );

    // Scroll hint (bottom-right corner of inner_history)
    if max_scroll > 0 {
        let hint = if app.chat.mode == ChatMode::Scroll { "↑j k↓" } else { "Esc:scroll" };
        let hint_style = if app.chat.mode == ChatMode::Scroll {
            Style::default().fg(p.accent)
        } else {
            Style::default().fg(p.dim)
        };
        let hint_w = hint.len() as u16 + 2;
        if inner_history.right() >= hint_w {
            let hint_area = Rect {
                x: inner_history.right() - hint_w,
                y: inner_history.bottom().saturating_sub(1),
                width: hint_w,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!(" {hint} "), hint_style))),
                hint_area,
            );
        }
    }

    // Input box — same H_PAD as history
    let inner_input = Rect {
        x: input_area.x + H_PAD,
        y: input_area.y,
        width: input_area.width.saturating_sub(H_PAD * 2),
        height: input_area.height,
    };

    let border_style = if app.chat.mode == ChatMode::Input {
        Style::default().fg(p.accent)
    } else {
        Style::default().fg(p.dim)
    };
    let mode_label = if app.chat.mode == ChatMode::Input { " INPUT " } else { " SCROLL " };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(mode_label, border_style.add_modifier(Modifier::BOLD)));

    let inner = input_block.inner(inner_input);
    f.render_widget(input_block, inner_input);

    let display_text = app.chat.input.display(app.chat.mode == ChatMode::Input);
    let is_placeholder = app.chat.input.value.is_empty();
    let text_style = if is_placeholder {
        Style::default().fg(p.dim)
    } else {
        Style::default().fg(p.text)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(display_text, text_style))),
        inner,
    );
}

fn render_message(
    msg: &ChatMessage,
    width: usize,
    p: &crate::theme::Palette,
    lines: &mut Vec<Line<'static>>,
) {
    match msg {
        ChatMessage::System { text } => {
            // System: no box, just dimmed italic with left indent
            lines.push(Line::from(Span::styled(
                format!(" {text}"),
                Style::default().fg(p.dim).add_modifier(Modifier::ITALIC),
            )));
            lines.push(Line::raw(""));
        }
        ChatMessage::User { text } => {
            box_top(lines, " you ", width, p.accent, p.dim);
            for l in text.lines() {
                box_line(lines, l, width, p.text, p.dim);
            }
            box_bottom(lines, width, p.dim);
            lines.push(Line::raw("")); // gap between messages
        }
        ChatMessage::Orbit { content } => {
            render_orbit_box(content, width, p, lines);
            lines.push(Line::raw("")); // gap between messages
        }
    }
}

fn render_orbit_box(
    content: &OrbitContent,
    width: usize,
    p: &crate::theme::Palette,
    lines: &mut Vec<Line<'static>>,
) {
    match content {
        OrbitContent::Text(text) => {
            box_top(lines, " orbit ", width, Color::Magenta, p.dim);
            for l in text.lines() {
                box_line(lines, l, width, p.text, p.dim);
            }
            box_bottom(lines, width, p.dim);
        }
        OrbitContent::Error(msg) => {
            box_top(lines, " orbit ", width, Color::Magenta, p.dim);
            box_spans(
                lines,
                vec![
                    Span::styled("✗  ", Style::default().fg(p.danger)),
                    Span::styled(msg.clone(), Style::default().fg(p.danger)),
                ],
                width,
                p.dim,
            );
            box_bottom(lines, width, p.dim);
        }
        OrbitContent::ScopeDetected { path, confidence } => {
            let conf_color = match confidence.as_str() {
                "High" => p.success,
                "Medium" => p.warning,
                _ => p.dim,
            };
            box_top(lines, " orbit — scope ", width, Color::Magenta, p.dim);
            box_spans(
                lines,
                vec![
                    Span::styled("→  ", Style::default().fg(p.label)),
                    Span::styled(path.clone(), Style::default().fg(p.text).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("  ({confidence})"), Style::default().fg(conf_color)),
                ],
                width,
                p.dim,
            );
            box_bottom(lines, width, p.dim);
        }
        OrbitContent::PlanInline { plan_id: _, nodes, done, failed } => {
            // Status determines the header label color, keeping the standard orbit message shape
            let (label, label_color) = if *failed {
                (" orbit — failed ", p.danger)
            } else if *done {
                (" orbit — done ", p.success)
            } else {
                (" orbit ", Color::Magenta)
            };
            box_top(lines, label, width, label_color, p.dim);
            for node in nodes {
                let (icon, icon_style) = node_icon(&node.status, p);
                let exec = node.agent.as_deref().or(node.executor.as_deref()).unwrap_or("ai");
                // label area: width − 2(borders) − 2(indent) − 3(icon+gap) − 2(gap) − exec.len()
                let label_avail = width.saturating_sub(2 + 2 + 3 + 2 + exec.len());
                let node_label = trunc(&node.label, label_avail);
                let pad = label_avail.saturating_sub(node_label.chars().count());
                box_spans(
                    lines,
                    vec![
                        Span::styled(icon, icon_style),
                        Span::raw("  "),
                        Span::styled(node_label, Style::default().fg(p.text)),
                        Span::raw(format!("{}  ", " ".repeat(pad))),
                        Span::styled(exec.to_string(), Style::default().fg(p.dim)),
                    ],
                    width,
                    p.dim,
                );
            }
            box_bottom(lines, width, p.dim);
        }
        OrbitContent::ApprovalNeeded { plan_id, node_id, label } => {
            box_top(lines, " orbit — approval needed ", width, Color::Magenta, p.dim);
            box_spans(
                lines,
                vec![
                    Span::styled("▶  ", Style::default().fg(p.warning).add_modifier(Modifier::BOLD)),
                    Span::styled(label.clone(), Style::default().fg(p.warning)),
                ],
                width,
                p.dim,
            );
            box_line(lines, &format!("   /approve {plan_id} {node_id}"), width, p.dim, p.dim);
            box_bottom(lines, width, p.dim);
        }
    }
}

// ── box drawing helpers ──────────────────────────────────────────────────────

/// ╭─ label ──────────────────────╮
fn box_top(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    width: usize,
    label_color: Color,
    border_color: Color,
) {
    // "╭─" (2) + label + fill*"─" + "╮" (1)  = width
    let label_len = label.chars().count();
    let fill = width.saturating_sub(2 + label_len + 1);
    lines.push(Line::from(vec![
        Span::styled("╭─", Style::default().fg(border_color)),
        Span::styled(label.to_string(), Style::default().fg(label_color).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{}╮", "─".repeat(fill)), Style::default().fg(border_color)),
    ]));
}

/// ╰──────────────────────────────╯
fn box_bottom(lines: &mut Vec<Line<'static>>, width: usize, border_color: Color) {
    lines.push(Line::from(Span::styled(
        format!("╰{}╯", "─".repeat(width.saturating_sub(2))),
        Style::default().fg(border_color),
    )));
}

/// │  text (padded to fill width)  │
fn box_line(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    width: usize,
    text_color: Color,
    border_color: Color,
) {
    // │(1) + "  "(2) + text + padding + │(1) = width  →  text+pad = width−4
    let content_w = width.saturating_sub(4);
    let display = trunc(text, content_w);
    let pad = content_w.saturating_sub(display.chars().count());
    lines.push(Line::from(vec![
        Span::styled("│", Style::default().fg(border_color)),
        Span::raw("  "),
        Span::styled(display, Style::default().fg(text_color)),
        Span::raw(" ".repeat(pad)),
        Span::styled("│", Style::default().fg(border_color)),
    ]));
}

/// │  [multiple styled spans] (padded) │
fn box_spans(
    lines: &mut Vec<Line<'static>>,
    content: Vec<Span<'static>>,
    width: usize,
    border_color: Color,
) {
    let content_w = width.saturating_sub(4); // │ + "  " + content + │
    let span_w: usize = content.iter().map(|s| s.content.chars().count()).sum();
    let pad = content_w.saturating_sub(span_w);
    let mut spans = vec![
        Span::styled("│", Style::default().fg(border_color)),
        Span::raw("  "),
    ];
    spans.extend(content);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled("│", Style::default().fg(border_color)));
    lines.push(Line::from(spans));
}

fn trunc(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > max && max > 0 {
        format!("{}…", chars[..max.saturating_sub(1)].iter().collect::<String>())
    } else {
        text.to_string()
    }
}

fn node_icon(status: &NodeStatus, p: &crate::theme::Palette) -> (String, Style) {
    match status {
        NodeStatus::Pending => ("○".into(), Style::default().fg(p.dim)),
        NodeStatus::Running => ("⟳".into(), Style::default().fg(p.warning).add_modifier(Modifier::BOLD)),
        NodeStatus::Completed => ("✓".into(), Style::default().fg(p.success)),
        NodeStatus::Failed => ("✗".into(), Style::default().fg(p.danger)),
        NodeStatus::Skipped => ("⊘".into(), Style::default().fg(p.dim)),
        NodeStatus::AwaitingApproval => {
            ("▶".into(), Style::default().fg(p.warning).add_modifier(Modifier::BOLD))
        }
    }
}
