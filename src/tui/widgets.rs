//! Shared TUI widgets.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

use crate::runtime::provider::TaskStatus;
use crate::state::RunStatus;

pub fn status_color_task(s: &TaskStatus) -> Color {
    match s {
        TaskStatus::Done => Color::Green,
        TaskStatus::Failed | TaskStatus::Timeout => Color::Red,
        TaskStatus::Running | TaskStatus::Starting => Color::Cyan,
        TaskStatus::Stopped | TaskStatus::Skipped => Color::DarkGray,
        TaskStatus::Pending | TaskStatus::Queued => Color::Yellow,
    }
}

pub fn status_color_run(s: &RunStatus) -> Color {
    match s {
        RunStatus::Completed => Color::Green,
        RunStatus::Failed | RunStatus::Aborted => Color::Red,
        RunStatus::Running | RunStatus::Validated => Color::Cyan,
        RunStatus::Paused => Color::Yellow,
        RunStatus::Init => Color::DarkGray,
    }
}

pub fn page_tabs(selected: usize) -> Tabs<'static> {
    let titles = vec![
        "1:Dash",
        "2:Graph",
        "3:Task",
        "4:Logs",
        "5:Term",
        "6:Help",
    ];
    Tabs::new(titles)
        .select(selected)
        .block(Block::default().borders(Borders::BOTTOM))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider(" ")
}

pub fn help_footer() -> Paragraph<'static> {
    Paragraph::new("q quit · Tab/1-6 pages · j/k or ↑↓ select · s stop task · o embed term · O external · r reload · ? help")
        .style(Style::default().fg(Color::DarkGray))
}
