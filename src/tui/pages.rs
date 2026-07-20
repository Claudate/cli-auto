//! TUI page renderers: Dashboard / Graph / Task / Logs / Terminals / Help.
//!
//! [INPUT]: RunState 快照 · 选中 task · term pane focus/zoom
//! [OUTPUT]: ratatui Frame 绘制
//! [POS]: tui 页面层
//! note: P2-5 Terminals = multi-pane log grid (pseudo-PTY); interactive write stays external
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/tui/CLAUDE.md

use std::path::PathBuf;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};

use super::app::{App, Page};
use super::widgets::{status_color_run, status_color_task};
use crate::graph::topo_layers;
use crate::plan::PlanIR;
use crate::runtime::provider::TaskStatus;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    match app.page {
        Page::Dashboard => render_dashboard(frame, app, area),
        Page::Graph => render_graph(frame, app, area),
        Page::Task => render_task(frame, app, area),
        Page::Logs => render_logs(frame, app, area),
        Page::Terminals => render_terminals(frame, app, area),
        Page::Help => render_help(frame, area),
    }
}

fn render_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(3)])
        .split(area);

    let st = &app.state;
    let total_cost: f64 = st.tasks.values().filter_map(|t| t.cost_usd).sum();
    let running = st
        .tasks
        .values()
        .filter(|t| matches!(t.status, TaskStatus::Running | TaskStatus::Starting))
        .count();
    let done = st
        .tasks
        .values()
        .filter(|t| matches!(t.status, TaskStatus::Done))
        .count();
    let failed = st
        .tasks
        .values()
        .filter(|t| matches!(t.status, TaskStatus::Failed | TaskStatus::Timeout))
        .count();

    let head = format!(
        "run {}  status {:?}  project {}\nplan {}\nadapter {}  tasks {}  running {}  done {}  failed {}  cost ${:.4}\nrun_dir {}",
        st.run_id,
        st.status,
        st.project_root.display(),
        st.plan_path.display(),
        st.adapter,
        st.tasks.len(),
        running,
        done,
        failed,
        total_cost,
        st.run_dir.display(),
    );
    frame.render_widget(
        Paragraph::new(head)
            .block(
                Block::default()
                    .title(" Dashboard ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(status_color_run(&st.status))),
            )
            .wrap(Wrap { trim: false }),
        chunks[0],
    );

    let mut ids: Vec<_> = st.tasks.keys().cloned().collect();
    ids.sort();
    let rows = ids.iter().enumerate().map(|(i, id)| {
        let t = &st.tasks[id];
        let style = if Some(i) == app.selected_task_idx {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(id.as_str()),
            Cell::from(format!("{:?}", t.status)).style(Style::default().fg(status_color_task(&t.status))),
            Cell::from(t.provider.as_str()),
            Cell::from(t.mode.as_str()),
            Cell::from(
                t.cost_usd
                    .map(|c| format!("{c:.3}"))
                    .unwrap_or_else(|| "—".into()),
            ),
            Cell::from(format!("{}", t.terminals.len())),
        ])
        .style(style)
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(6),
        ],
    )
    .header(
        Row::new(vec!["id", "status", "provider", "mode", "cost", "terms"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().title(" Tasks ").borders(Borders::ALL));
    frame.render_widget(table, chunks[1]);
}

fn render_graph(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(plan) = &app.plan {
        let layers = topo_layers(plan);
        for (i, layer) in layers.iter().enumerate() {
            lines.push(Line::from(format!("stage {i}:")));
            for id in layer {
                let st = app
                    .state
                    .tasks
                    .get(id)
                    .map(|t| format!("{:?}", t.status))
                    .unwrap_or_else(|| "?".into());
                let color = app
                    .state
                    .tasks
                    .get(id)
                    .map(|t| status_color_task(&t.status))
                    .unwrap_or(Color::White);
                let mark = if app.selected_task_id().as_deref() == Some(id.as_str()) {
                    ">"
                } else {
                    " "
                };
                let title = plan.task(id).map(|t| t.title.as_str()).unwrap_or("");
                lines.push(Line::from(vec![
                    Span::raw(format!("{mark} {id} ")),
                    Span::styled(st, Style::default().fg(color)),
                    Span::raw(format!("  {title}")),
                ]));
            }
            lines.push(Line::from(""));
        }
    } else {
        lines.push(Line::from(
            "(plan.resolved.json not loaded — showing task list only)",
        ));
        for id in app.sorted_task_ids() {
            lines.push(Line::from(id));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(" Graph ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_task(frame: &mut Frame, app: &App, area: Rect) {
    let Some(id) = app.selected_task_id() else {
        frame.render_widget(
            Paragraph::new("no task selected").block(
                Block::default()
                    .title(" Task ")
                    .borders(Borders::ALL),
            ),
            area,
        );
        return;
    };
    let t = &app.state.tasks[&id];
    let prompt = app
        .state
        .task_dir(&id)
        .join("prompt.md");
    let prompt_text = std::fs::read_to_string(prompt).unwrap_or_else(|_| "(no prompt snapshot)".into());
    let meta = std::fs::read_to_string(app.state.task_dir(&id).join("meta.json"))
        .unwrap_or_else(|_| "{}".into());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Percentage(45), Constraint::Min(3)])
        .split(area);

    let head = format!(
        "task {id}\nstatus {:?}  provider {}  mode {}\ncost {:?}  session {:?}  agent {:?}\nwork_dir {}\nbranch {:?}  pid {:?}  terminals {:?}",
        t.status,
        t.provider,
        t.mode,
        t.cost_usd,
        t.session_id,
        t.agent_id,
        t.work_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".into()),
        t.worktree_branch,
        t.pid,
        t.terminals,
    );
    frame.render_widget(
        Paragraph::new(head).block(Block::default().title(" Task ").borders(Borders::ALL)),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(prompt_text)
            .block(Block::default().title(" Prompt ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(meta)
            .block(Block::default().title(" Meta ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        chunks[2],
    );
}

fn render_logs(frame: &mut Frame, app: &App, area: Rect) {
    let text = if let Some(id) = app.selected_task_id() {
        let p = app.state.task_dir(&id).join("stdout.json");
        let err = app.state.task_dir(&id).join("stderr.log");
        let mut s = std::fs::read_to_string(p).unwrap_or_default();
        let e = std::fs::read_to_string(err).unwrap_or_default();
        if !e.is_empty() {
            s.push_str("\n--- stderr ---\n");
            s.push_str(&e);
        }
        if s.is_empty() {
            // fall back to events
            std::fs::read_to_string(app.state.events_path()).unwrap_or_else(|_| "(no logs)".into())
        } else {
            s
        }
    } else {
        std::fs::read_to_string(app.state.events_path()).unwrap_or_else(|_| "(no events)".into())
    };
    // show tail
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(area.height.saturating_sub(2) as usize);
    let body = lines[start..].join("\n");
    let title = app
        .selected_task_id()
        .map(|id| format!(" Logs · {id} "))
        .unwrap_or_else(|| " Logs · events ".into());
    frame.render_widget(
        Paragraph::new(body)
            .block(Block::default().title(title).borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// P2-5: multi-pane log grid (pseudo-PTY for print/bg workers).
/// Interactive keyboard→PTY write is still external-terminal only (O).
fn render_terminals(frame: &mut Frame, app: &App, area: Rect) {
    let panes = app.open_term_panes();
    if panes.is_empty() {
        let hint = "No open terminal sessions.\n\n\
  o  open embedded log pane for selected task (registry + live tail)\n\
  O  open external system terminal (interactive attach)\n\
  n/p  cycle panes · z zoom · x close selected task sessions\n\n\
Workers in print/bg mode have no interactive PTY — panes show stdout tail.";
        frame.render_widget(
            Paragraph::new(hint)
                .block(
                    Block::default()
                        .title(" Terminals · empty ")
                        .borders(Borders::ALL),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }

    let focus = app.selected_term_idx.min(panes.len().saturating_sub(1));
    if app.term_zoom {
        render_term_pane(frame, app, panes[focus], area, true);
        return;
    }

    // Grid: 1 → full; 2 → 1×2; 3–4 → 2×2; 5–6 → 2×3; else clamp to 6.
    let n = panes.len().min(6);
    let (cols, rows) = match n {
        1 => (1, 1),
        2 => (2, 1),
        3 | 4 => (2, 2),
        _ => (3, 2),
    };
    let row_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Ratio(1, rows as u32); rows])
        .split(area);
    for r in 0..rows {
        let col_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, cols as u32); cols])
            .split(row_chunks[r]);
        for c in 0..cols {
            let i = r * cols + c;
            if i >= n {
                frame.render_widget(
                    Block::default().borders(Borders::ALL).title(" — "),
                    col_chunks[c],
                );
                continue;
            }
            render_term_pane(frame, app, panes[i], col_chunks[c], i == focus);
        }
    }
}

fn render_term_pane(
    frame: &mut Frame,
    app: &App,
    session: &crate::terminal::TerminalSession,
    area: Rect,
    focused: bool,
) {
    let kind = match session.kind {
        crate::terminal::SessionKind::Embedded => "embed",
        crate::terminal::SessionKind::External => "ext",
    };
    let mark = if focused { "▶" } else { " " };
    let zoom = if app.term_zoom { " · ZOOM" } else { "" };
    let title = format!(
        " {mark} {} · {kind} · {}{zoom} ",
        session.task_id,
        &session.id[..session.id.len().min(8)]
    );

    let body = term_pane_body(session, area);
    let block = if focused {
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
    } else {
        Block::default().title(title).borders(Borders::ALL)
    };
    frame.render_widget(
        Paragraph::new(body)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn term_pane_body(session: &crate::terminal::TerminalSession, area: Rect) -> String {
    let h = area.height.saturating_sub(2) as usize;
    let w = area.width.saturating_sub(2) as usize;
    let path = session
        .log_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(&session.command));
    let raw = if path.is_file() {
        std::fs::read_to_string(&path).unwrap_or_else(|e| format!("(read error: {e})"))
    } else if matches!(session.kind, crate::terminal::SessionKind::External) {
        format!(
            "(external terminal)\nlauncher={}\ncwd={}\ncmd={}",
            session.launcher.as_deref().unwrap_or("—"),
            session.cwd.display(),
            session.command
        )
    } else {
        "(no log file yet — worker may not have started)".into()
    };
    // Strip crude ANSI CSI for a clean pane (full VT parse is not this slice).
    let cleaned = strip_ansi_lite(&raw);
    let lines: Vec<&str> = cleaned.lines().collect();
    let start = lines.len().saturating_sub(h.max(1));
    lines[start..]
        .iter()
        .map(|l| {
            let t: String = l.chars().take(w.max(8)).collect();
            t
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Drop common CSI / OSC sequences without a full vt100 crate.
fn strip_ansi_lite(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    while let Some(n) = chars.next() {
                        if n.is_ascii_alphabetic() || n == '@' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    // OSC … BEL or ST
                    while let Some(n) = chars.next() {
                        if n == '\u{7}' {
                            break;
                        }
                        if n == '\u{1b}' {
                            let _ = chars.next(); // eat \
                            break;
                        }
                    }
                }
                Some(_) => {
                    let _ = chars.next();
                }
                None => {}
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn render_help(frame: &mut Frame, area: Rect) {
    let text = r#"cco TUI · multi-page observer

Pages: 1 Dashboard  2 Graph  3 Task  4 Logs  5 Terminals  6 Help

Keys:
  q / Esc     quit TUI (does not kill run; Esc also unzooms Terminals)
  Tab         next page
  1-6         jump page
  j / ↓       next task
  k / ↑       prev task
  s           stop selected task (best-effort kill / .done)
  o           open embedded log pane (stdout tail grid · P2-5)
  O           open external terminal window for selected task
  x           close open terminal sessions of selected task
  n / p / ←→  cycle terminal panes (Terminals page)
  z           zoom focused pane (Terminals page)
  r           reload state from disk
  ?           this help

Architecture:
  TUI only observes ~/.cco/runs/<id> (+ terminals.json).
  Scheduler runs independently; use headless `cco run` or `cco tui <run_id>`.
  Terminals grid = multi-pane log tail (pseudo-PTY); interactive write → external (O).
"#;
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().title(" Help ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

#[allow(dead_code)]
pub fn plan_summary(plan: &PlanIR) -> String {
    format!("{} ({} tasks)", plan.name, plan.tasks.len())
}
