# src/terminal/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: re-export TerminalManager / SessionKind / ExternalLauncher / windows_cmdline_preview
manager.rs: 会话登记 open/list/close · follow_logs · embedded|external
external.rs: launcher 探测 + spawn 分发 — macOS Terminal/iTerm · Linux xdg · kitty/wezterm/ghostty/tmux/custom · 委托 Windows 变体
win.rs: **P2-7** Windows 薄切片 — wt / powershell / cmd open + cmdline dry-run preview

法则: 成员完整·一行一文件·父级链接·技术词前置
注: CLI/TUI/桌面 open_task_terminal 已接（P1-2）；Windows 外开 = wt → powershell → cmd 探测序；follow_logs 在 Win 用 Get-Content -Wait

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
