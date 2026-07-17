# cco — CLI Orchestrator（项目任务控制台）
Rust + Tokio + Clap + Tauri 2 + 原生 web（HTML/CSS/JS）

<directory>
src/ — 核心库与 CLI（plan/runtime/doctor/services/tui）
src-tauri/ — 桌面壳（Tauri commands）
web/ — 桌面前端资源（打包进 App）
docs/ — 产品/UX 计划文档
examples/ — 示例计划
tests/ — 集成与金样
scripts/ — 打包脚本
dist/ — 已打包 CCO.app
</directory>

<config>
Cargo.toml — workspace（cco + cco-desktop）
claude-cli-orchestrator-plan.md — 编排器设计真源
docs/desktop-ux-redesign-plan.md — 桌面壳 UX
docs/product-mode-b-ai-planner.md — 模式 B（AI 规划）
docs/ux-simple-mainpath-2026-07-17.md — 易用性主路径简化
</config>

法则: 极简·稳定·导航·版本精确

[PROTOCOL]: 架构变更时更新此文件
