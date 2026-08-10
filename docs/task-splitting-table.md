# 任务拆分台

## 概述
任务拆分台是 cco 的核心 UI 组件，允许用户与 AI 共同拆分计划为可执行任务，提供交互式确认、范围管理、依赖关系维护。

## 核心功能
- AI 智能拆分（Mode B）
- 用户手动编辑任务
- 确认并启动执行
- 进度监控与巡检

## 实现
- 前端：web/ 中的 split 相关组件
- 后端：src/app/split.rs

[PROTOCOL]: 本文**无** P0–P5 / 平行总落地勾选。残余债 **S2–S6 只认** 存储文文末。改行为先读本文 + 存储文；细节与金样回 archive。

## 存储真源
- [`cco-split-format-sqlite-2026-07-21.md`](../cco-split-format-sqlite-2026-07-21.md)

/**
 * [INPUT]: 依赖 `cco-split-format-sqlite-2026-07-21.md` 的 SoT 字段和 split-product-rules.md 的行为契约；依赖 src/app/split.rs 的 SplitUseCase
 * [OUTPUT]: 对外提供任务拆分台 UI 组件、confirm_start 契约、SQLite 索引查询
 * [POS]: docs/ 的 L2 模块地图；被 src/app/ 和 web/ 消费，作为 Mode B 唯一开跑入口
 * [PROTOCOL]: 变更时更新此头部，然后检查 /docs/CLAUDE.md
 */
