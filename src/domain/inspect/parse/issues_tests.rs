    use super::*;

    #[test]
    fn parse_issues_grades_severity() {
        let text = r#"
- id: I-1
  severity=map
  plan_ref: §8 GEB
  path: CLAUDE.md
  symptom: L1 still says 待验
  fix_wp: Update CLAUDE.md config row to F0+F1 closed

- id: I-2 severity=blocking plan_ref=S5 path=web/
  symptom: desktop Chinese path not verified
  fix_wp: Re-run GUI or mark DEGRADED only if plan allows

- id: I-3
  severity: residual
  plan_ref: F2
  symptom: optional polish
"#;
        let parsed = parse_issues_text(text);
        assert!(parsed.len() >= 3, "parsed={parsed:?}");
        let i1 = parsed.iter().find(|i| i.id.contains("I-1")).unwrap();
        assert_eq!(i1.severity, IssueSeverity::Map);
        assert!(i1.severity.is_blocking_for_gate());
        let i2 = parsed.iter().find(|i| i.id.contains("I-2")).unwrap();
        assert_eq!(i2.severity, IssueSeverity::Blocking);
        let i3 = parsed.iter().find(|i| i.id.contains("I-3")).unwrap();
        assert_eq!(i3.severity, IssueSeverity::Residual);
        assert!(!i3.severity.is_blocking_for_gate());
    }

    #[test]
    fn markdown_bold_severity_residual_not_blocking() {
        let text = r#"
### I-1
- **severity**: residual
- **plan_ref**: 验收
- **fix_wp**: polish
- **说明**: archive soft 历史表
"#;
        let parsed = parse_issues_text(text);
        assert!(!parsed.is_empty(), "parsed={parsed:?}");
        let i1 = parsed.iter().find(|i| i.id.contains("I-1")).unwrap();
        assert_eq!(i1.severity, IssueSeverity::Residual);
        assert!(!i1.severity.is_blocking_for_gate());
    }

    #[test]
    fn markdown_bold_severity_out_of_scope() {
        let text = "- **severity**: out-of-scope\n- **plan_ref**: 后置\n";
        let parsed = parse_issues_text(text);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].severity, IssueSeverity::OutOfScope);
    }

    #[test]
    fn parse_issues_fail_closed_without_severity() {
        let parsed = parse_issues_text("- missing plan pointer in CLAUDE.md\n");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].severity, IssueSeverity::Blocking);
    }

    #[test]
    fn real_t7_issues_markdown_all_non_blocking() {
        let text = r#"
# ISSUES · t7-inspect

plan_ref: docs/chat §验收
Result companion: VERDICT.md → **PASS**

## residual

### I-1
- **severity**: residual
- **plan_ref**: S2–S6
- **fix_wp**: polish
- **说明**: archive soft 历史表

### I-2
- **severity**: residual
- **plan_ref**: 死链
- **fix_wp**: polish

## out-of-scope

### I-4
- **severity**: out-of-scope
- **plan_ref**: 后置

## 空集确认

- **blocking**: 无
- **map**: 无
"#;
        let parsed = parse_issues_text(text);
        assert_eq!(parsed.len(), 3, "parsed={parsed:?}");
        assert!(
            parsed.iter().all(|i| !i.severity.is_blocking_for_gate()),
            "parsed={parsed:?}"
        );
        assert!(parsed.iter().any(|i| i.id.contains("I-1")));
        assert!(parsed.iter().any(|i| i.id.contains("I-2")));
        assert!(parsed.iter().any(|i| i.id.contains("I-4")));
    }

    /// Regression: inspect often writes `### R1` residual + `out-of-scope（中文说明）`.
    /// Host used to fail-closed the oos line as Blocking → false P-loop gate fail.
    #[test]
    fn residual_r_headers_and_oos_fullwidth_note_not_blocking() {
        let text = r#"# ISSUES · t6 inspect

> Result 为 PASS：无 blocking / map。

## residual

### R1 · 场景「茶席」线标为 Lucide coffee 杯形
- **severity:** residual
- **plan_ref:** §做.4
- **fix_wp:** t4
- **描述:** 线标命名略西式

### R2 · ys-006 材质字面
- **severity:** residual
- **plan_ref:** §做.3
- **fix_wp:** t2

### R3 · 静态 href
- **severity:** residual
- **plan_ref:** 主路径
- **fix_wp:** t5

## blocking

（无）

## map

（无 · L1/L2 不同构未发现）

## out-of-scope

### O1 · 浏览器实机
- **severity:** out-of-scope（本波角色=静态 inspect；任务允许无浏览器时静态完成）
- **plan_ref:** 任务大纲 7
- **fix_wp:** 人工
- **描述:** 未跑 npm run dev
"#;
        let parsed = parse_issues_text(text);
        assert_eq!(parsed.len(), 4, "parsed={parsed:?}");
        assert!(
            parsed.iter().all(|i| !i.severity.is_blocking_for_gate()),
            "blocking false-positive: {parsed:?}"
        );
        assert_eq!(
            count_blocking_for_test(&parsed),
            0,
            "blocking_n must be 0 for residual+oos-only"
        );
        let o1 = parsed.iter().find(|i| i.id.starts_with('O')).unwrap();
        assert_eq!(o1.severity, IssueSeverity::OutOfScope);
        assert!(
            parsed
                .iter()
                .filter(|i| i.severity == IssueSeverity::Residual)
                .count()
                >= 3
        );
    }

    #[test]
    fn severity_token_strips_fullwidth_chinese_note() {
        assert_eq!(
            severity_from_token("out-of-scope（本波角色=静态 inspect）"),
            Some(IssueSeverity::OutOfScope)
        );
        assert_eq!(
            severity_from_token("residual (optional polish)"),
            Some(IssueSeverity::Residual)
        );
        // Unknown bare token does not invent Blocking at token layer.
        assert_eq!(severity_from_token("mystery-grade"), None);
    }

    /// Regression: reinspect used `### issue_id=R1` + prose mentioning severity=blocking
    /// in the preamble. Host used to merge the whole file into one Blocking issue.
    #[test]
    fn issue_id_heading_and_prose_severity_not_blocking() {
        let text = r#"# ISSUES · reinspect-r1

Companion: VERDICT.md → Result: **PASS**
规则：存在 **open** severity=blocking 或 severity=map（map 默认 blocking）则不得 PASS。
本表 **open blocking=0 · open map=0** → 允许 PASS。

## Open issues（仅 residual）

### issue_id=R1
- severity: residual
- plan_ref: git 卫生
- path: inkos-rs/**
- symptom: 业务源码未 commit
- fix_wp: commit wave

### issue_id=R2
- severity: residual
- plan_ref: GUI
- path: ContinuityDrawer.tsx
- symptom: 无录像
- fix_wp: optional

### issue_id=R3
- severity: residual
- plan_ref: gitignore
- path: .gitignore
- symptom: 未 staged
- fix_wp: stage

## Closed this reinspect
| B6 | was-blocking | closed |
"#;
        let parsed = parse_issues_text(text);
        assert_eq!(parsed.len(), 3, "parsed={parsed:?}");
        assert!(
            parsed.iter().all(|i| i.severity == IssueSeverity::Residual),
            "parsed={parsed:?}"
        );
        assert_eq!(count_blocking_for_test(&parsed), 0);
        assert!(parsed.iter().any(|i| i.id == "R1"));
        assert!(parsed.iter().any(|i| i.id == "R2"));
        assert!(parsed.iter().any(|i| i.id == "R3"));
    }

    fn count_blocking_for_test(issues: &[ParsedIssue]) -> usize {
        issues
            .iter()
            .filter(|i| i.severity.is_blocking_for_gate())
            .count()
    }

    /// Fix A: "out of scope" in symptom *prose* describes a scope violation, not a
    /// grade — must fail-closed to Blocking. A line-start declared grade (`- out-of-scope`)
    /// still yields OutOfScope.
    #[test]
    fn prose_out_of_scope_is_blocking_not_oos_grade() {
        // Reported fixture: scope-leak described with "(out of scope)" in prose.
        let prose = r#"- file: examples/demo_b/extra.rs
- symptom: written by feat-a (out of scope)
- suggestion: remove file; re-run feat-a within scope
"#;
        let parsed = parse_issues_text(prose);
        assert_eq!(parsed.len(), 1, "parsed={parsed:?}");
        assert_eq!(
            parsed[0].severity,
            IssueSeverity::Blocking,
            "prose 里的 out of scope 不应判成 OOS grade: {parsed:?}"
        );
        assert!(parsed[0].severity.is_blocking_for_gate());

        // Declared line-start grade still works.
        let declared = r#"- out-of-scope: scope-locked demo_b region
- file: examples/demo_b/extra.rs
"#;
        let parsed2 = parse_issues_text(declared);
        assert!(!parsed2.is_empty(), "parsed2={parsed2:?}");
        assert!(
            parsed2
                .iter()
                .all(|i| i.severity == IssueSeverity::OutOfScope),
            "declared out-of-scope 应判 OOS: {parsed2:?}"
        );
    }
