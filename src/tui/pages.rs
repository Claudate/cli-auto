//! Page renderers.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Wrap};

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

fn render_terminals(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .term_sessions
        .iter()
        .map(|s| {
            let mark = if s.closed { "closed" } else { "open" };
            ListItem::new(format!(
                "{}  [{mark}]  task={}  {:?}  {}",
                s.id,
                s.task_id,
                s.kind,
                s.log_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| s.command.chars().take(40).collect())
            ))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .title(" Terminals (o embed · O external · x close selected session) ")
            .borders(Borders::ALL),
    );
    frame.render_widget(list, area);

    // optional log preview for first open session of selected task
    // (kept simple: list only; M3.1 can split panes)
}

fn render_help(frame: &mut Frame, area: Rect) {
    let text = r#"cco TUI · multi-page observer

Pages: 1 Dashboard  2 Graph  3 Task  4 Logs  5 Terminals  6 Help

Keys:
  q / Esc     quit TUI (does not kill run)
  Tab         next page
  1-6         jump page
  j / ↓       next task
  k / ↑       prev task
  s           stop selected task (best-effort kill / .done)
  o           open embedded terminal session (log follow registry)
  O           open external terminal window for selected task
  x           close first open terminal of selected task
  r           reload state from disk
  ?           this help

Architecture:
  TUI only observes ~/.cco/runs/<id> (+ terminals.json).
  Scheduler runs independently; use headless `cco run` or `cco tui <run_id>`.
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
