# src/tui/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: re-export run_tui / TuiOptions
app.rs: 事件循环 · 轮询 run 目录 · 快捷键 · **P2-5** selected_term_idx / term_zoom / open_term_panes
pages.rs: Dashboard/Graph/Task/Logs/Terminals（多窗格 log 网格 · zoom · strip_ansi_lite）/Help
widgets.rs: 共用渲染小组件

法则: 成员完整·一行一文件·父级链接·技术词前置
注: Terminals 网格 = stdout 只读伪 PTY；真交互 PTY write 仍走外部终端 (O)，不引入 portable-pty

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
