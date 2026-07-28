//! System / user prompts for ModelSplitAgent (cco-split/v1 only).
//!
//! [INPUT]: plan markdown · max_parallel · project label · optional grain_hint
//! [OUTPUT]: prompt strings (no IO)
//! [POS]: plan/split_agent
//! [PROTOCOL]: 字段与 domain/plan/cco_split/types 同步；改 schema 先改类型与 docs 附录 A
//! note: Q2 要求 scope_paths + 派工 body 模板；禁止 worker 腔；并行=文件所有权；W4 grain 可选一句

/// System role: Plan Mode only — never write product code.
pub fn system_prompt() -> String {
    let base = r#"你是 cco 的计划拆分 Agent（Plan Mode / OpenHands 气质）。
你的唯一工作：把 Markdown 计划拆成可并行、可验收的工作步骤图。你不写业务代码、不改仓库、不探索全库。

输出要求：
- 只输出一个 JSON 对象（可包在 ```json 代码块里），禁止解释性前后文。
- schema 必须是 "cco-split/v1"。
- 顶层字段：schema, title, max_parallel, tasks[]。
- tasks[] 每项字段：
  id, title, summary, body, depends_on, optional, enabled, kind, done_when, plan_ref,
  can_parallel, scope_paths
- kind 仅允许：do | check | system
- depends_on 只写真实先后依赖的 id；计划表写「依赖：无」则 []；禁止为凑波次伪造边
- optional=true 时 enabled 默认 false（留给人勾选）；必选步骤 enabled=true
- max_parallel 是同时路数上限
- can_parallel=true 仅当与可能同批的步骤 **scope_paths 无写冲突** 且无硬依赖
- title 用待办语气（动词开头），不要目录名/文件名当唯一标题；去掉 ☐/✅
- summary 一句话人话结果（给非开发看）；**禁止**「你是执行…worker」口吻
- done_when 对齐计划「完成定义 / 验收」；可观察、可勾选
- **scope_paths（必填语义）**：本步可写文件/目录 glob 列表。
  · 有代码/配置改动 → 列出互不抢的路径（如 web/js/features/result/**）
  · 纯文档/勾选 → ["docs/**"] 或明确单文件
  · 确无路径 → [] 且 body 首行写「无代码路径」
  · 同文件写者不得 can_parallel=true
- body 是给「后续执行 AI」的完整说明，**必须**用下列小标题（中文）：
  【做什么】一句话结果
  【改哪里】与 scope_paths 一致
  【怎样算做完】可观察标准
  【先等谁】无则写「无；可与 … 并行」
  【不要做什么】计划非目标/硬契约（禁止旁路确认开跑、禁止删用户磁盘等）
  【自测】2–4 条
  不要写完整业务实现代码；不要输出「你是 worker」脚手架
- 禁止把：修订历史、非目标清单本身、PROTOCOL、纯目录说明、空话口号 拆成任务
- 禁止输出 provider/role（高级字段留给人在拆分台填）；**必须**输出 scope_paths
- 一步一个可验收结果；任务数宜 3–12，除非计划明确更多
- **验收/巡检任务 ≠ 关账**：check 类只对照计划写 VERDICT/ISSUES 语义；**禁止**在 title/body 写「并回写台账 / commit / 勾选进度」——台账关账由 host `sys-closeout` 注入，勿揉进巡检一步
"#;
    // Product delivery + recipes + backend + layout + color + type + copy + motion.
    format!(
        "{base}{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}",
        crate::domain::chat::split_agent_delivery_guidance(),
        crate::domain::chat::ui_delivery_recipes_guidance(),
        crate::domain::chat::backend_architecture_guidance(),
        crate::domain::chat::ui_layout_systems_guidance(),
        crate::domain::chat::ui_color_systems_guidance(),
        crate::domain::chat::ui_typography_systems_guidance(),
        crate::domain::chat::ui_copy_systems_guidance(),
        crate::domain::chat::ui_motion_effects_guidance()
    )
}

/// User message with plan body and runtime caps.
///
/// `grain_hint`: optional one-line 偏粗/偏细 preference (W4); empty/None omitted.
/// `clarify_depth`: optional clarification style; none|soft1(soft)2(full_opt).
/// `revision_notes`: optional free-text replan feedback; empty/None omitted.
/// `repo_digest`: optional host-built shallow tree (read-only); empty/None omitted.
pub fn user_prompt(
    project_label: &str,
    max_parallel: usize,
    plan_md: &str,
    grain_hint: Option<&str>,
    clarify_depth: Option<&str>,
    revision_notes: Option<&str>,
    repo_digest: Option<&str>,
) -> String {
    let grain_line = grain_hint
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("粒度偏好：{s}\n"))
        .unwrap_or_default();
    // Map clarify depth to Chinese instructions.
    let clarify_line = match clarify_depth.map(str::trim).filter(|s| !s.is_empty()).as_deref() {
        Some("soft1") => "拆分前如信息不足，最多提一个软澄清问题（不阻塞）\n".to_string(),
        Some("soft2") => "拆分前如信息不足，可列至多两个软澄清问题（不阻塞）\n".to_string(),
        Some("full_opt") => "可列出完整的可选澄清问题（范围/假设），但不得阻塞拆分\n".to_string(),
        Some("none") | None => String::new(),
        _ => String::new(),
    };
    let revision_block = revision_notes
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            format!("用户对上次拆分的反馈（优先满足；与硬约束冲突时仍守硬约束）：\n{s}\n\n")
        })
        .unwrap_or_default();
    let digest_block = repo_digest
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("{s}\n\n"))
        .unwrap_or_default();
    format!(
        r#"项目：{project_label}
同时路数上限 max_parallel：{max_parallel}
{grain_line}{clarify_line}{revision_block}{digest_block}硬约束（写入每条 body 的「不要做什么」若相关）：
- 唯一开跑入口是人在拆分台确认；禁止设计旁路开跑
- soft-fill 不得静默覆盖任务已显式指定的执行方式
- 完成 = 对照计划验收，不是进程 exit 0
- 并行单位 = 文件/模块所有权（scope_paths），不是「波次数字」
- 若有仓库浅览：scope_paths 优先用真实存在的路径，勿编造未出现目录

请将下列计划拆成 cco-split/v1 JSON（仅 JSON）。
优先信计划正文里的「依赖 / 完成定义 / 落点」表；无依赖的步骤不要串成一条链。
若上方有用户反馈：在依赖、scope、粒度、标题上优先按反馈改，不要无视后原样重拆。

----- PLAN START -----
{plan_md}
----- PLAN END -----
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_requires_scope_and_body_template() {
        let s = system_prompt();
        assert!(s.contains("scope_paths"), "must require scope_paths");
        assert!(s.contains("【做什么】"), "body template");
        assert!(
            !s.contains("不要输出 provider/role/scope"),
            "old forbid-scope line must be gone"
        );
        assert!(s.contains("你是 worker") || s.contains("worker"), "ban worker tone");
        assert!(
            s.contains("western-saas") || s.contains("色系"),
            "color systems guidance must append"
        );
        assert!(
            s.contains("交付深度") || s.contains("MVC") || s.contains("演示"),
            "backend architecture guidance must append"
        );
        assert!(
            s.contains("font-display")
                || s.contains("文楷")
                || s.contains("字体")
                || s.contains("LXGW"),
            "typography guidance must append"
        );
        assert!(
            s.contains("prefers-reduced-motion")
                || s.contains("动效档")
                || s.contains("GSAP")
                || s.contains("anime"),
            "motion effects guidance must append"
        );
        assert!(
            s.contains("marketing")
                || s.contains("portfolio")
                || s.contains("站点类型")
                || s.contains("信息结构"),
            "layout systems guidance must append"
        );
        assert!(
            s.contains("R-overseas") || s.contains("配方") || s.contains("R-tool"),
            "delivery recipes must append"
        );
        assert!(
            s.contains("R-ios")
                || s.contains("R-material")
                || s.contains("ios-hig")
                || s.contains("material"),
            "platform style recipes must append"
        );
        assert!(
            s.contains("主 CTA")
                || s.contains("空态")
                || s.contains("界面文案")
                || s.contains("Lorem"),
            "ui copy systems must append"
        );
    }

    #[test]
    fn user_mentions_parallel_ownership() {
        let u = user_prompt("/proj", 2, "# hi", None, None, None, None);
        assert!(u.contains("max_parallel：2"));
        assert!(u.contains("scope_paths") || u.contains("文件"));
        assert!(!u.contains("粒度偏好"));
        assert!(!u.contains("用户对上次拆分的反馈"));
    }

    #[test]
    fn user_includes_grain_when_set() {
        let u = user_prompt("/proj", 2, "# hi", Some("偏细：步骤拆开"), None, None, None);
        assert!(u.contains("粒度偏好：偏细"));
        assert!(u.contains("拆分前如信息不足，最多提一个软澄清问题（不阻塞）") || !u.contains("soft1"), "clarify should be omitted for soft1 unless set");
    }

    #[test]
    fn user_includes_clarify_depth_when_set() {
        // none: omit
        let u = user_prompt("/proj", 2, "# hi", None, Some("none"), None, None);
        assert!(!u.contains("拆分前如信息不足"));
        // soft1: one question hint
        let u = user_prompt("/proj", 2, "# hi", None, Some("soft1"), None, None);
        assert!(u.contains("拆分前如信息不足，最多提一个软澄清问题（不阻塞）"));
        // full_opt: all optional questions hint
        let u = user_prompt("/proj", 2, "# hi", None, Some("full_opt"), None, None);
        assert!(u.contains("可列出完整的可选澄清问题（范围/假设），但不得阻塞拆分"));
    }

    #[test]
    fn user_includes_revision_notes_when_set() {
        let u = user_prompt(
            "/proj",
            2,
            "# hi",
            None,
            None,
            Some("合并文案与 SEO 为一步；scope 不要抢 index.html"),
            None,
        );
        assert!(u.contains("用户对上次拆分的反馈"));
        assert!(u.contains("合并文案与 SEO"));
        assert!(u.contains("优先按反馈改"));
    }

    #[test]
    fn user_includes_repo_digest_when_set() {
        let u = user_prompt(
            "/proj",
            2,
            "# hi",
            None,
            None,
            None,
            Some("仓库浅览：\n顶层：src/ · web/"),
        );
        assert!(u.contains("仓库浅览"));
        assert!(u.contains("真实存在的路径"));
    }
}
