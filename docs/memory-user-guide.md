# 语义记忆 · 使用指南

> 面向使用者（PM / 出海 / 非开发）。开发细节见 [`memory-dev-guide.md`](./memory-dev-guide.md)；落地勾选见 [`agentmemory-integration-plan-2026-08-12.md`](./agentmemory-integration-plan-2026-08-12.md)。

## 它是什么

cco 可以**记住你项目里发生过的事**，下次不用重复解释：

- 上次这个项目是怎么拆分任务的 → 再拆同类项目时自动带上参考
- 哪个引擎在哪类任务上老是超时 → 连续踩坑后自动预防性换引擎

全部数据存在**你自己的电脑上**（`~/.cco/memory/`），不上传任何云端，不需要安装 Docker 或 Node.js。

## 默认关闭 · 如何开启

记忆功能默认是**关**的（开着和关着行为完全一致，不影响任何现有流程）。开启方式：编辑 `~/.cco/config.toml`：

```toml
[memory]
enabled = true
```

## 可选：更聪明的语义检索

不装模型也能用（按关键词全文检索）。想让检索"懂意思"（比如问"上次 Tauri 项目怎么拆的"能找到没写 Tauri 字样的记录），下载两个文件放到 `~/.cco/models/`：

| 文件 | 来源 |
|------|------|
| `all-MiniLM-L6-v2.onnx` | [HuggingFace sentence-transformers/all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)（`onnx/model.onnx`，下载后改名） |
| `tokenizer.json` | 同上仓库根目录 |

放好后重启 cco 即生效，单次检索实测约 13ms，无感知延迟。

## 开启后它会做什么

1. **拆分参考**：AI 拆分成功后自动记一笔；下次拆分时检索最相关的 3 条历史，作为「仅参考」注入——不构成硬约束，最终方案仍由你确认。
2. **任务结果记录**：每个任务跑完（成功 / 超时 / 失败）自动记录引擎和角色。
3. **预防性换引擎**：某个引擎在同类任务上有 **3 次以上记录且失败率超过 30%** 时，下次开跑前自动换备选引擎，并在日志和结果台注明原因。**你手动指定的引擎永远不会被换**。

## 数据管理

- **位置**：`~/.cco/memory/`（SQLite 数据库 + 检索索引）
- **自动清理**：超过 90 天的记录自动删除；总量超 1 万条时删最旧的（均可在 `[memory]` 段调整 `ttl_days` / `max_entries`）
- **彻底清空**：关闭 cco 后删除 `~/.cco/memory/` 目录即可
- **一键关闭**：`enabled = false`，立即回到无记忆行为
