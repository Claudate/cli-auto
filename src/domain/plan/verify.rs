//! Heuristic: is a string a host-runnable shell verify command?
//!
//! [INPUT]: trim 后 acceptance / verify_cmd 候选
//! [OUTPUT]: is_runnable_verify → bool
//! [POS]: domain/plan · 调度 / convert 分流用；**禁止** web 复制
//! [PROTOCOL]: 变更时更新此头部与 src/domain/CLAUDE.md
//!
//! 策略：**宁可漏跑真命令，不可误跑人话**（H0）。

/// Whether `s` looks like a one-line (or short) shell command safe to `sh -c`.
///
/// Human criteria (Chinese/English prose, multi-line narratives) return `false`.
/// Only explicit shell-shaped strings return `true`.
pub fn is_runnable_verify(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    // Multi-line Chinese/English narrative is never shell.
    if s.contains('\n') {
        return lines_all_shell_shaped(s);
    }
    line_looks_like_shell(s)
}

/// Alias used in plan docs / older naming.
#[inline]
pub fn looks_like_shell_acceptance(s: &str) -> bool {
    is_runnable_verify(s)
}

fn lines_all_shell_shaped(s: &str) -> bool {
    let mut any = false;
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        any = true;
        if !line_looks_like_shell(t) {
            return false;
        }
    }
    any
}

fn line_looks_like_shell(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    // CJK prose without shell prefix → not runnable.
    if contains_cjk(s) && !has_shell_prefix(s) {
        return false;
    }
    // Long English prose without shell markers (e.g. "file exists", "must PASS").
    if s.len() > 80 && !has_shell_prefix(s) && !has_shell_operator(s) {
        return false;
    }
    // Space-free pure words that aren't known bare commands → not shell.
    if !s.contains(|c: char| c.is_whitespace() || "[]$|&;<>(){}*\"'`./\\=".contains(c)) {
        // bare: true / false / exitN rare; allow true|false only
        return matches!(s, "true" | "false");
    }
    if has_shell_prefix(s) {
        return true;
    }
    // `[ -f path ]` style test builtin
    if s.starts_with('[') {
        return true;
    }
    // `./script.sh` or path ending .sh
    if s.starts_with("./") || s.starts_with("../") || s.ends_with(".sh") {
        return true;
    }
    // exit N
    if s.starts_with("exit ") || s == "exit" {
        return true;
    }
    // Operators often mean real shell (but reject pure Chinese + &&)
    if has_shell_operator(s) && !contains_cjk(s) {
        return true;
    }
    false
}

fn has_shell_prefix(s: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "test ",
        "cargo ",
        "npm ",
        "pnpm ",
        "yarn ",
        "make ",
        "sh ",
        "bash ",
        "zsh ",
        "git ",
        "grep ",
        "rg ",
        "find ",
        "ls ",
        "cat ",
        "echo ",
        "printf ",
        "which ",
        "command ",
        "python ",
        "python3 ",
        "node ",
        "rustc ",
        "go ",
        "curl ",
        "wget ",
        "diff ",
        "cmp ",
        "stat ",
        "wc ",
        "xargs ",
        "env ",
        "cd ",
        "mkdir ",
        "rm ",
        "cp ",
        "mv ",
        "chmod ",
        "true",
        "false",
        "exit ",
        "if ",
        "for ",
        "while ",
        "set ",
        "unset ",
        "export ",
        "source ",
        ". ",
    ];
    let lower = s.to_ascii_lowercase();
    PREFIXES.iter().any(|p| {
        if *p == "true" || *p == "false" {
            lower == *p || lower.starts_with(&format!("{p} ")) || lower.starts_with(&format!("{p};"))
        } else {
            lower.starts_with(p)
        }
    })
}

fn has_shell_operator(s: &str) -> bool {
    s.contains("&&")
        || s.contains("||")
        || s.contains("|")
        || s.contains(">")
        || s.contains('<')
        || s.contains("$(")
        || s.contains("${")
        || s.contains("`")
}

fn contains_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        // CJK Unified + extensions + fullwidth forms commonly used in prose
        (0x4E00..=0x9FFF).contains(&u)
            || (0x3400..=0x4DBF).contains(&u)
            || (0xF900..=0xFAFF).contains(&u)
            || (0x3000..=0x303F).contains(&u)
            || (0xFF00..=0xFFEF).contains(&u)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_human_criteria_not_shell() {
        assert!(!is_runnable_verify(
            "存在 .cco-out/inspect/VERDICT.md 与 ISSUES.md；阻塞项必须 FAIL"
        ));
        assert!(!is_runnable_verify(
            "有变更则已 commit+push，或明确说明无变更/失败原因"
        ));
        assert!(!is_runnable_verify(
            "已开 PR 并输出 CCO_PR_OK url=…，或明确 CCO_PR_SKIPPED reason=…"
        ));
        assert!(!is_runnable_verify("步骤完成后页面可打开且无红错"));
        assert!(!is_runnable_verify(""));
        assert!(!is_runnable_verify("   "));
    }

    #[test]
    fn english_prose_not_shell() {
        assert!(!is_runnable_verify("file exists"));
        assert!(!is_runnable_verify("must PASS inspection"));
        assert!(!is_runnable_verify("VERDICT is PASS and ISSUES empty"));
    }

    #[test]
    fn real_shell_commands_yes() {
        assert!(is_runnable_verify("test -f MARKER.txt"));
        assert!(is_runnable_verify("[ -f MARKER.txt ]"));
        assert!(is_runnable_verify("exit 1"));
        assert!(is_runnable_verify("cargo test -p cco"));
        assert!(is_runnable_verify("npm test"));
        assert!(is_runnable_verify("pnpm run build"));
        assert!(is_runnable_verify("./scripts/check.sh"));
        assert!(is_runnable_verify("sh scripts/smoke.sh"));
        assert!(is_runnable_verify("true"));
        assert!(is_runnable_verify("git status --porcelain"));
        assert!(is_runnable_verify("test -f a && test -f b"));
    }

    #[test]
    fn multiline_chinese_no() {
        assert!(!is_runnable_verify(
            "存在 VERDICT\n与 ISSUES\n阻塞项必须 FAIL"
        ));
    }

    #[test]
    fn multiline_shell_yes() {
        assert!(is_runnable_verify("test -f a\ntest -f b"));
    }
}
