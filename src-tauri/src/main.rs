//! cco-desktop binary entry.
//!
//! [INPUT]: 无
//! [OUTPUT]: cco_desktop_lib::run()
//! [POS]: 桌面进程入口
//! [PROTOCOL]: 变更时更新此头部，然后检查 src-tauri/CLAUDE.md

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    cco_desktop_lib::run();
}
