//! TUI application loop.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use super::pages;
use super::widgets::{help_footer, page_tabs};
use crate::config::Config;
use crate::plan::PlanIR;
use crate::state::RunState;
use crate::terminal::{SessionKind, TerminalManager, TerminalSession};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Graph,
    Task,
    Logs,
    Terminals,
    Help,
}

impl Page {
    fn idx(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Graph => 1,
            Self::Task => 2,
            Self::Logs => 3,
            Self::Terminals => 4,
            Self::Help => 5,
        }
    }

    fn from_idx(i: usize) -> Self {
        match i % 6 {
            0 => Self::Dashboard,
            1 => Self::Graph,
            2 => Self::Task,
            3 => Self::Logs,
            4 => Self::Terminals,
            _ => Self::Help,
        }
    }
}

pub struct TuiOptions {
    pub run_dir: PathBuf,
    pub tick_ms: u64,
    pub launcher: String,
    pub custom_command: Option<String>,
    pub max_embedded: usize,
    pub max_external: usize,
}

pub struct App {
    pub state: RunState,
    pub plan: Option<PlanIR>,
    pub page: Page,
    pub selected_task_idx: Option<usize>,
    pub term_sessions: Vec<TerminalSession>,
    pub message: String,
    pub tm: TerminalManager,
    pub should_quit: bool,
}

impl App {
    pub fn load(opts: &TuiOptions) -> Result<Self> {
        let state = RunState::load(&opts.run_dir)?;
        let plan = load_resolved_plan(&opts.run_dir);
        let tm = TerminalManager::for_run(
            &opts.run_dir,
            &opts.launcher,
            opts.custom_command.clone(),
        )
        .with_limits(opts.max_embedded, opts.max_external);
        let term_sessions = tm.list().unwrap_or_default();
        let mut app = Self {
            state,
            plan,
            page: Page::Dashboard,
            selected_task_idx: Some(0),
            term_sessions,
            message: String::new(),
            tm,
            should_quit: false,
        };
        app.clamp_selection();
        Ok(app)
    }

    pub fn reload(&mut self) -> Result<()> {
        let dir = self.state.run_dir.clone();
        self.state = RunState::load(&dir)?;
        self.plan = load_resolved_plan(&dir);
        self.term_sessions = self.tm.list().unwrap_or_default();
        self.clamp_selection();
        self.message = format!("reloaded {}", chrono::Local::now().format("%H:%M:%S"));
        Ok(())
    }

    pub fn sorted_task_ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = self.state.tasks.keys().cloned().collect();
        ids.sort();
        ids
    }

    pub fn selected_task_id(&self) -> Option<String> {
        let ids = self.sorted_task_ids();
        self.selected_task_idx.and_then(|i| ids.get(i).cloned())
    }

    fn clamp_selection(&mut self) {
        let n = self.state.tasks.len();
        if n == 0 {
            self.selected_task_idx = None;
            return;
        }
        match self.selected_task_idx {
            None => self.selected_task_idx = Some(0),
            Some(i) if i >= n => self.selected_task_idx = Some(n - 1),
            _ => {}
        }
    }

    fn select_next(&mut self) {
        let n = self.state.tasks.len();
        if n == 0 {
            return;
        }
        let i = self.selected_task_idx.unwrap_or(0);
        self.selected_task_idx = Some((i + 1) % n);
    }

    fn select_prev(&mut self) {
        let n = self.state.tasks.len();
        if n == 0 {
            return;
        }
        let i = self.selected_task_idx.unwrap_or(0);
        self.selected_task_idx = Some(if i == 0 { n - 1 } else { i - 1 });
    }

    fn stop_selected(&mut self) {
        let Some(id) = self.selected_task_id() else {
            return;
        };
        if let Some(ts) = self.state.tasks.get(&id) {
            if let Some(pid) = ts.pid {
                kill_pid(pid);
            }
        }
        let meta = self.state.task_dir(&id).join("meta.json");
        if let Ok(text) = std::fs::read_to_string(&meta) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(pid) = v.get("pid").and_then(|p| p.as_u64()) {
                    kill_pid(pid as u32);
                }
                if let Some(agent) = v.get("agent_id").and_then(|a| a.as_str()) {
                    let bin = std::env::var("CCO_CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
                    let _ = std::process::Command::new(bin).args(["stop", agent]).status();
                }
            }
        }
        let _ = std::fs::write(self.state.task_dir(&id).join(".done"), "130");
        if let Some(ts) = self.state.tasks.get_mut(&id) {
            ts.status = crate::runtime::provider::TaskStatus::Stopped;
            ts.finished_at = Some(chrono::Utc::now());
        }
        let _ = self.state.save();
        let _ = self.tm.close_task(&id);
        self.message = format!("stopped task {id}");
        let _ = self.reload();
    }

    fn open_term(&mut self, kind: SessionKind) {
        let Some(id) = self.selected_task_id() else {
            return;
        };
        let task_dir = self.state.task_dir(&id);
        let mut cwd = self.state.project_root.clone();
        let wd = task_dir.join("work_dir.json");
        if let Ok(text) = std::fs::read_to_string(wd) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(p) = v.get("work_dir").and_then(|x| x.as_str()) {
                    cwd = PathBuf::from(p);
                }
            }
        }
        let stdout = task_dir.join("stdout.json");
        let stderr = task_dir.join("stderr.log");
        if !stdout.exists() {
            let _ = std::fs::write(&stdout, "");
        }
        if !stderr.exists() {
            let _ = std::fs::write(&stderr, "");
        }
        match self
            .tm
            .open_follow_logs(&id, &cwd, &stdout, &stderr, kind)
        {
            Ok(s) => {
                if let Some(ts) = self.state.tasks.get_mut(&id) {
                    ts.terminals.push(s.id.clone());
                }
                let _ = self.state.save();
                self.message = format!("opened {:?} session {}", kind, s.id);
            }
            Err(e) => self.message = format!("term open failed: {e:#}"),
        }
        let _ = self.reload();
    }

    fn close_term_for_task(&mut self) {
        let Some(id) = self.selected_task_id() else {
            return;
        };
        match self.tm.close_task(&id) {
            Ok(n) => self.message = format!("closed {n} session(s) for {id}"),
            Err(e) => self.message = format!("close failed: {e:#}"),
        }
        let _ = self.reload();
    }
}

pub fn run_tui(opts: TuiOptions) -> Result<()> {
    let mut app = App::load(&opts)?;
    enable_raw_mode().context("enable_raw_mode")?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).context("EnterAlternateScreen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick = Duration::from_millis(opts.tick_ms.max(50));
    let result = loop_ui(&mut terminal, &mut app, tick);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn loop_ui(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    tick: Duration,
) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if event::poll(tick)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                handle_key(app, key.code, key.modifiers);
            }
        } else {
            // periodic reload
            let _ = app.reload();
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Tab => app.page = Page::from_idx(app.page.idx() + 1),
        KeyCode::BackTab => app.page = Page::from_idx(app.page.idx() + 5),
        KeyCode::Char('1') => app.page = Page::Dashboard,
        KeyCode::Char('2') => app.page = Page::Graph,
        KeyCode::Char('3') => app.page = Page::Task,
        KeyCode::Char('4') => app.page = Page::Logs,
        KeyCode::Char('5') => app.page = Page::Terminals,
        KeyCode::Char('6') | KeyCode::Char('?') => app.page = Page::Help,
        KeyCode::Char('j') | KeyCode::Down => app.select_next(),
        KeyCode::Char('k') | KeyCode::Up => app.select_prev(),
        KeyCode::Char('s') => app.stop_selected(),
        KeyCode::Char('o') => app.open_term(SessionKind::Embedded),
        KeyCode::Char('O') => app.open_term(SessionKind::External),
        KeyCode::Char('x') => app.close_term_for_task(),
        KeyCode::Char('r') => {
            let _ = app.reload();
        }
        _ => {}
    }
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(page_tabs(app.page.idx()), chunks[0]);
    pages::render(frame, app, chunks[1]);
    let msg = if app.message.is_empty() {
        format!("cco tui · {}", app.state.run_id)
    } else {
        app.message.clone()
    };
    frame.render_widget(
        Paragraph::new(msg).block(Block::default().borders(Borders::TOP)),
        chunks[2],
    );
    frame.render_widget(help_footer(), chunks[3]);
}

fn load_resolved_plan(run_dir: &Path) -> Option<PlanIR> {
    let p = run_dir.join("plan.resolved.json");
    let text = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(&text).ok()
}

fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            let _ = kill(pid as i32, 15);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

/// Build TuiOptions from config + run dir.
pub fn options_from_config(run_dir: PathBuf, config: &Config) -> TuiOptions {
    TuiOptions {
        run_dir,
        tick_ms: config.tui.tick_ms,
        launcher: config.terminal.external_launcher.clone(),
        custom_command: config.terminal.external_command.clone(),
        max_embedded: config.terminal.max_embedded,
        max_external: config.terminal.max_external,
    }
}
