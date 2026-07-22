# src/report/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: write_reports(report.md+json · 完整骨架：摘要·对照计划·步骤结果·花费与用时·后续·备注 · 人话 H1 · headline · plan_compare JSON) · plan_short_name · report_headline · report_summary_line · summarize_providers · handoff_paths · format_status_by_provider · print_report_md
fallback.rs: build_plan_compare · fill_plan_compare · PlanCompareSection/Kind · format_elapsed_human · follow_up_lines · render_plan_compare_md（无 VERDICT 占位不伪造 PASS；复用 handoff inspect_loop_view；**P0-4** headline 与 web inspectCopy 同词；**P2-1** verification 副栏「原计划要验收」）
write_tests.rs: cfg(test) write_reports 骨架/fallback/FAIL 单测

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
