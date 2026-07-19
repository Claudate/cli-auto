# src/plan/adapters/
> L2 | 父级: /src/plan/CLAUDE.md

成员清单
mod.rs: 子模块导出 cco_v1 / raw_single / serial_prompts
cco_v1.rs: schema cco-plan/v1 YAML → PlanIR（role/scope/outputs/require_inspect 可选，serde 向后兼容；P2-1 inspect defaults 在 load_plan 物化）
serial_prompts.rs: serial-prompts/v0 多段 Markdown → 串行任务
raw_single.rs: 任意文本单任务兜底适配器

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md
