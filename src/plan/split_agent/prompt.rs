//! System / user prompts for ModelSplitAgent (cco-split/v1 only).
//!
//! [INPUT]: plan markdown · max_parallel · project label
//! [OUTPUT]: prompt strings (no IO)
//! [POS]: plan/split_agent
//! [PROTOCOL]: 字段与 domain/plan/cco_split/types 同步；改 schema 先改类型与 docs 附录 A

/// System role: Plan Mode only — never write product code.
pub fn system_prompt() -> &'static str {
    r#"你是 cco 的计划拆分 Agent（Plan Mode / OpenHands 气质）。
你的唯一工作：把 Markdown 计划拆成可执行工作步骤图。你不写业务代码、不改仓库、不探索全库。

输出要求：
- 只输出一个 JSON 对象（可包在 ```json 代码块里），禁止解释性前后文。
- schema 必须是 "cco-split/v1"。
- 顶层字段：schema, title, max_parallel, tasks[]。
- tasks[] 每项字段：
  id, title, summary, body, depends_on, optional, enabled, kind, done_when, plan_ref, can_parallel
- kind 仅允许：do | check | system
- depends_on 只写真实先后依赖的 id；无依赖用 []
- optional=true 时 enabled 默认 false（留给人勾选）；必选步骤 enabled=true
- max_parallel 是同时路数上限，禁止为凑波次伪造 depends_on
- can_parallel=true 表示与同波兄弟无硬依赖时可并行
- title 用待办语气（动词开头），不要用目录名/文件名当标题
- body 是给「后续执行 AI」的说明：做什么、完成标准、注意点；不要写完整业务实现代码
- 禁止把：修订历史、非目标、PROTOCOL、纯目录说明、空话口号 拆成任务
- 一步一个可验收结果；任务数宜 3–12，除非计划明确更多
- 不要输出 provider/role/scope（高级字段留给人在拆分台填）
"#
}

/// User message with plan body and runtime caps.
pub fn user_prompt(project_label: &str, max_parallel: usize, plan_md: &str) -> String {
    format!(
        r#"项目：{project_label}
同时路数上限 max_parallel：{max_parallel}

请将下列计划拆成 cco-split/v1 JSON（仅 JSON）：

----- PLAN START -----
{plan_md}
----- PLAN END -----
"#
    )
}
