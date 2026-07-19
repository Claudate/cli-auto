/**
 * [INPUT]: 依赖 Tauri invoke / dialog，消费 services 暴露的桌面命令
 * [OUTPUT]: 项目任务控制台 UI 状态机（选计划→分配→监视）
 * [POS]: web/ 前端入口（D4：实现已纵切到 web/js/{{state,plan,monitor,log,doctor}}.js）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 加载顺序（见 index.html）：
 *   js/state.js → js/plan.js → js/monitor.js → js/log.js → js/chat.js → js/doctor.js
 */
/* cco desktop — entry (logic in web/js/) */
