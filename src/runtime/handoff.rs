//! Host-owned run handoff ledger (handoff.md + handoff.json).
//!
//! [INPUT]: run_dir · PlanIR · RunState · task terminal results
//! [OUTPUT]: handoff.md / handoff.json；outputs 缺失检查；inspect VERDICT/ISSUES 分级(P-loop)；
//!           REWORK_HOOK · build_rework_plan · accept_residual · inspect_loop_view；[CCO_HANDOFF] 前缀
//! [POS]: 事中账本；仅 host 写入，worker 只写自己 fragment
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::plan::{PlanIR, TaskIR, TaskRole};
use crate::runtime::provider::{TaskResult, TaskStatus};
use crate::state::{RunState, RunStatus};

pub const HANDOFF_SCHEMA: &str = "cco-handoff/v1";

/// Conventional inspect verdict product (relative to work_dir / project_root).
pub const INSPECT_VERDICT_REL: &str = ".cco-out/inspect/VERDICT.md";
/// Conventional inspect issues product for rework consumption (P2-3).
pub const INSPECT_ISSUES_REL: &str = ".cco-out/inspect/ISSUES.md";

/// Classification of inspect VERDICT product (P2-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectVerdict {
    Pass,
    Fail,
    /// No verdict file, or content does not clearly say PASS/FAIL.
    Unknown,
}

/// Marker wrapping the host-injected handoff summary on task start (P1-5).
pub const HANDOFF_PROMPT_OPEN: &str = "[CCO_HANDOFF]";
pub const HANDOFF_PROMPT_CLOSE: &str = "[/CCO_HANDOFF]";

/// Max chars for each depends_on fragment summary inside the prompt prefix.
const PREFIX_SUMMARY_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    pub schema: String,
    pub run_id: String,
    pub updated: DateTime<Utc>,
    pub project: String,
    pub plan: String,
    pub status: String,
    pub board: Vec<BoardRow>,
    #[serde(default)]
    pub timeline: Vec<String>,
    #[serde(default)]
    pub fragments: BTreeMap<String, Fragment>,
    #[serde(default)]
    pub open_risks: Vec<String>,
    #[serde(default)]
    pub instructions_for_next: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardRow {
    pub id: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub status: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Fragment {
    pub status: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
}

impl Handoff {
    pub fn path_md(run_dir: &Path) -> PathBuf {
        run_dir.join("handoff.md")
    }

    pub fn path_json(run_dir: &Path) -> PathBuf {
        run_dir.join("handoff.json")
    }

    /// Empty shell at run start: Board = all pending.
    pub fn init_shell(plan: &PlanIR, state: &RunState) -> Self {
        let board = plan
            .tasks
            .iter()
            .map(|t| board_row_from_task(t, "pending", None, String::new()))
            .collect();
        Self {
            schema: HANDOFF_SCHEMA.into(),
            run_id: state.run_id.clone(),
            updated: Utc::now(),
            project: state.project_root.display().to_string(),
            plan: state.plan_path.display().to_string(),
            status: "running".into(),
            board,
            timeline: vec![format!(
                "{} · run_start · shell created",
                Utc::now().to_rfc3339()
            )],
            fragments: BTreeMap::new(),
            open_risks: vec![],
            instructions_for_next: default_next_instructions(plan, &[]),
        }
    }

    pub fn load(run_dir: &Path) -> Result<Self> {
        let path = Self::path_json(run_dir);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, run_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(run_dir)?;
        let json_path = Self::path_json(run_dir);
        std::fs::write(
            &json_path,
            serde_json::to_string_pretty(self)?,
        )
        .with_context(|| format!("write {}", json_path.display()))?;
        let md_path = Self::path_md(run_dir);
        std::fs::write(&md_path, self.render_md())
            .with_context(|| format!("write {}", md_path.display()))?;
        Ok(())
    }

    pub fn render_md(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# CCO Handoff · run_id={}\n", self.run_id));
        md.push_str(&format!("updated: {}\n", self.updated.to_rfc3339()));
        md.push_str(&format!("project: {}\n", self.project));
        md.push_str(&format!("plan: {}\n", self.plan));
        md.push_str(&format!("status: {}\n\n", self.status));

        md.push_str("## Board\n");
        md.push_str("| id | provider | role | status | scope | outputs | cost | notes |\n");
        md.push_str("|----|----------|------|--------|-------|---------|------|-------|\n");
        for r in &self.board {
            let role = r.role.as_deref().unwrap_or("-");
            let cost = r
                .cost
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "-".into());
            let outs = if r.outputs.is_empty() {
                "-".into()
            } else {
                r.outputs.join(", ")
            };
            let scope = if r.scope.is_empty() {
                "-"
            } else {
                r.scope.as_str()
            };
            let notes = if r.notes.is_empty() { "-" } else { r.notes.as_str() };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                r.id, r.provider, role, r.status, scope, outs, cost, notes
            ));
        }
        md.push('\n');

        md.push_str("## Timeline\n");
        if self.timeline.is_empty() {
            md.push_str("- (empty)\n");
        } else {
            for line in &self.timeline {
                md.push_str(&format!("- {line}\n"));
            }
        }
        md.push('\n');

        md.push_str("## Fragments\n");
        if self.fragments.is_empty() {
            md.push_str("_none yet_\n");
        } else {
            for (id, f) in &self.fragments {
                md.push_str(&format!("### {id}\n"));
                md.push_str(&format!(
                    "- status: {} · provider: {}\n",
                    f.status, f.provider
                ));
                if let Some(wd) = &f.work_dir {
                    md.push_str(&format!("- work_dir: `{wd}`\n"));
                }
                if let Some(br) = &f.branch {
                    md.push_str(&format!("- branch: `{br}`\n"));
                }
                if !f.summary.is_empty() {
                    md.push_str(&format!("- summary: {}\n", f.summary));
                }
                if !f.artifacts.is_empty() {
                    md.push_str(&format!("- artifacts: {}\n", f.artifacts.join(", ")));
                }
                if !f.risks.is_empty() {
                    md.push_str(&format!("- risks: {}\n", f.risks.join("; ")));
                }
                md.push('\n');
            }
        }

        md.push_str("## Open risks\n");
        if self.open_risks.is_empty() {
            md.push_str("- (none)\n");
        } else {
            for r in &self.open_risks {
                md.push_str(&format!("- {r}\n"));
            }
        }
        md.push('\n');

        md.push_str("## Instructions for next worker\n");
        if self.instructions_for_next.is_empty() {
            md.push_str("- read Board + Fragments; respect scope\n");
        } else {
            md.push_str(&self.instructions_for_next);
            if !self.instructions_for_next.ends_with('\n') {
                md.push('\n');
            }
        }
        md
    }

    fn set_board_status(&mut self, task_id: &str, status: &str, cost: Option<f64>, notes: &str) {
        if let Some(row) = self.board.iter_mut().find(|r| r.id == task_id) {
            row.status = status.into();
            if cost.is_some() {
                row.cost = cost;
            }
            if !notes.is_empty() {
                row.notes = notes.into();
            }
        }
    }

    fn push_timeline(&mut self, line: impl Into<String>) {
        self.timeline.push(line.into());
        // keep timeline bounded
        if self.timeline.len() > 200 {
            let drop_n = self.timeline.len() - 200;
            self.timeline.drain(0..drop_n);
        }
    }
}

fn role_str(role: Option<TaskRole>) -> Option<String> {
    role.map(|r| {
        match r {
            TaskRole::Scout => "scout",
            TaskRole::Implement => "implement",
            TaskRole::Integrate => "integrate",
            TaskRole::Inspect => "inspect",
        }
        .to_string()
    })
}

fn scope_summary(task: &TaskIR) -> String {
    let Some(s) = &task.scope else {
        return String::new();
    };
    let mut parts = Vec::new();
    if !s.paths.is_empty() {
        parts.push(format!("w:{}", s.paths.join(",")));
    }
    if !s.readonly.is_empty() {
        parts.push(format!("r:{}", s.readonly.join(",")));
    }
    if !s.forbid.is_empty() {
        parts.push(format!("!:{}", s.forbid.join(",")));
    }
    parts.join(" ")
}

fn board_row_from_task(
    task: &TaskIR,
    status: &str,
    cost: Option<f64>,
    notes: String,
) -> BoardRow {
    BoardRow {
        id: task.id.clone(),
        provider: task.provider.clone(),
        role: role_str(task.role),
        status: status.into(),
        scope: scope_summary(task),
        outputs: task.outputs.clone(),
        cost,
        notes,
    }
}

fn default_next_instructions(plan: &PlanIR, done: &[String]) -> String {
    let pending: Vec<&str> = plan
        .tasks
        .iter()
        .filter(|t| !done.iter().any(|d| d == &t.id))
        .map(|t| t.id.as_str())
        .collect();
    if pending.is_empty() {
        "- all tasks terminal; no next worker\n".into()
    } else {
        format!(
            "- ready candidates (check depends_on): {}\n- read Board + Fragments of depends_on\n- write only your outputs; do not edit global handoff\n",
            pending.join(", ")
        )
    }
}

fn status_label(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Pending => "pending",
        TaskStatus::Queued => "queued",
        TaskStatus::Starting => "starting",
        TaskStatus::Running => "running",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
        TaskStatus::Timeout => "timeout",
        TaskStatus::Stopped => "stopped",
        TaskStatus::Skipped => "skipped",
    }
}

/// Resolve output path relative to work_dir, then project_root.
pub fn resolve_output_path(rel: &str, work_dir: &Path, project_root: &Path) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let in_work = work_dir.join(p);
    if in_work.exists() {
        return in_work;
    }
    project_root.join(p)
}

/// If TaskIR.outputs is non-empty, require each file to exist when task claims Done.
/// Returns Ok(missing) list (empty = all present). Empty outputs → Ok([]).
pub fn missing_outputs(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> Vec<String> {
    if task.outputs.is_empty() {
        return vec![];
    }
    task.outputs
        .iter()
        .filter(|o| {
            let path = resolve_output_path(o, work_dir, project_root);
            !path.is_file() && !path.is_dir()
        })
        .cloned()
        .collect()
}

// ── P2-3 / P-loop: inspect VERDICT + graded ISSUES + rework ───────────

/// Default max rework waves after inspect FAIL / blocking ISSUES (P-loop Q5).
pub const REWORK_MAX_ROUNDS: u32 = 2;

/// Map-class rework may only touch GEB/docs pointers (P-loop Q2/Q3; inspect still read-only).
pub const MAP_REWORK_PATH_WHITELIST: &[&str] = &[
    "CLAUDE.md",
    "docs/CLAUDE.md",
    "docs/gap-and-landing-plan-2026-07-18.md",
    "docs/plan-execute-inspect-rework-2026-07-19.md",
    "docs/**",
    ".cco-out/**",
];

/// ISSUE severity grades (P-loop §3.4.3). `map` defaults to blocking for host gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    Blocking,
    Map,
    Residual,
    OutOfScope,
}

impl IssueSeverity {
    /// Host gate: blocks plan-loop success unless residual/out-of-scope or user accepts residual.
    pub fn is_blocking_for_gate(self) -> bool {
        matches!(self, IssueSeverity::Blocking | IssueSeverity::Map)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            IssueSeverity::Blocking => "blocking",
            IssueSeverity::Map => "map",
            IssueSeverity::Residual => "residual",
            IssueSeverity::OutOfScope => "out-of-scope",
        }
    }
}

/// One structured ISSUES row (best-effort parse; free-form still works).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIssue {
    pub id: String,
    pub severity: IssueSeverity,
    pub plan_ref: String,
    pub path: String,
    pub symptom: String,
    pub fix_wp: String,
    /// Original line(s) for rework prompt paste.
    pub raw: String,
}

fn looks_like_verdict_path(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    lower.contains("verdict")
}

fn looks_like_issues_path(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    lower.contains("issues")
}

/// Candidate VERDICT paths: declared outputs that look like VERDICT, plus convention for role=inspect.
pub fn verdict_candidate_paths(task: &TaskIR) -> Vec<String> {
    let mut out: Vec<String> = task
        .outputs
        .iter()
        .filter(|o| looks_like_verdict_path(o))
        .cloned()
        .collect();
    // role=inspect always checks conventional path even if not listed in outputs.
    if task.role == Some(TaskRole::Inspect) && !out.iter().any(|o| o == INSPECT_VERDICT_REL) {
        out.push(INSPECT_VERDICT_REL.into());
    }
    out
}

/// Candidate ISSUES paths for rework consumption.
pub fn issues_candidate_paths(task: &TaskIR) -> Vec<String> {
    let mut out: Vec<String> = task
        .outputs
        .iter()
        .filter(|o| looks_like_issues_path(o))
        .cloned()
        .collect();
    if task.role == Some(TaskRole::Inspect) && !out.iter().any(|o| o == INSPECT_ISSUES_REL) {
        out.push(INSPECT_ISSUES_REL.into());
    }
    // Fallback: if VERDICT convention was used, also try ISSUES convention.
    if out.is_empty()
        && task
            .outputs
            .iter()
            .any(|o| looks_like_verdict_path(o) || o == INSPECT_VERDICT_REL)
    {
        out.push(INSPECT_ISSUES_REL.into());
    }
    out
}

/// Parse raw VERDICT text: first clear PASS/FAIL wins (line-oriented, then whole body).
pub fn parse_verdict_text(text: &str) -> InspectVerdict {
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // Prefer first meaningful line: "FAIL" / "PASS" or "VERDICT: FAIL"
        let upper = t.to_ascii_uppercase();
        // Word-boundary style: avoid matching FAIL inside longer tokens poorly.
        if upper == "FAIL"
            || upper.starts_with("FAIL ")
            || upper.starts_with("FAIL:")
            || upper.starts_with("FAIL|")
            || upper.contains("VERDICT=FAIL")
            || upper.contains("VERDICT: FAIL")
            || upper.contains("VERDICT:FAIL")
            || upper.contains("RESULT: FAIL")
            || upper.contains("RESULT:FAIL")
            || upper.contains("**RESULT: FAIL**")
        {
            return InspectVerdict::Fail;
        }
        if upper == "PASS"
            || upper.starts_with("PASS ")
            || upper.starts_with("PASS:")
            || upper.starts_with("PASS|")
            || upper.contains("VERDICT=PASS")
            || upper.contains("VERDICT: PASS")
            || upper.contains("VERDICT:PASS")
            || upper.contains("RESULT: PASS")
            || upper.contains("RESULT:PASS")
            || upper.contains("**RESULT: PASS**")
        {
            return InspectVerdict::Pass;
        }
        // First non-empty line had content but neither — keep scanning body below.
        break;
    }
    let upper = text.to_ascii_uppercase();
    // Whole-body fallback: FAIL takes precedence if both appear.
    let has_fail = upper.split_whitespace().any(|w| {
        w == "FAIL" || w.starts_with("FAIL:") || w.starts_with("FAIL|")
    }) || upper.contains("VERDICT=FAIL")
        || upper.contains("VERDICT: FAIL")
        || upper.contains("VERDICT:FAIL");
    let has_pass = upper.split_whitespace().any(|w| {
        w == "PASS" || w.starts_with("PASS:") || w.starts_with("PASS|")
    }) || upper.contains("VERDICT=PASS")
        || upper.contains("VERDICT: PASS")
        || upper.contains("VERDICT:PASS");
    if has_fail {
        InspectVerdict::Fail
    } else if has_pass {
        InspectVerdict::Pass
    } else {
        InspectVerdict::Unknown
    }
}

/// Read inspect VERDICT product; Unknown if no file / unparseable.
pub fn read_inspect_verdict(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> InspectVerdict {
    let candidates = verdict_candidate_paths(task);
    if candidates.is_empty() {
        return InspectVerdict::Unknown;
    }
    for rel in candidates {
        let path = resolve_output_path(&rel, work_dir, project_root);
        if !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let v = parse_verdict_text(&text);
        if v != InspectVerdict::Unknown {
            return v;
        }
    }
    InspectVerdict::Unknown
}

/// Read raw ISSUES.md text (first existing candidate).
pub fn read_inspect_issues_text(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> Option<String> {
    for rel in issues_candidate_paths(task) {
        let path = resolve_output_path(&rel, work_dir, project_root);
        if !path.is_file() {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Some(text);
        }
    }
    None
}

/// Parse ISSUES body into graded rows (P-loop §3.4.3).
///
/// Recognizes:
/// - `severity=blocking|map|residual|out-of-scope` (or `severity: …`)
/// - `plan_ref=` / `path=` / `fix_wp=` / `- id: I-*`
/// - Free-form bullets without severity → **blocking** (fail-closed for silent residual).
pub fn parse_issues_text(text: &str) -> Vec<ParsedIssue> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    let lower_all = trimmed.to_ascii_lowercase();
    if matches!(
        lower_all.as_str(),
        "无" | "none" | "n/a" | "na" | "no issues" | "no issue"
    ) {
        return vec![];
    }

    // Split into issue blocks: lines starting with `- id:` / `## I-` / `- I-` start a new block;
    // otherwise treat each non-empty bullet as its own issue.
    let mut blocks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for line in trimmed.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let starts_block = t.starts_with("- id:")
            || t.starts_with("-id:")
            || t.starts_with("## I-")
            || t.starts_with("### I-")
            || (t.starts_with("- I-") || t.starts_with("* I-"))
            || (t.starts_with('-') && t.contains("severity="))
            || (t.starts_with('-') && t.contains("severity:"));
        if starts_block && !cur.is_empty() {
            blocks.push(cur.trim().to_string());
            cur.clear();
        }
        if !cur.is_empty() {
            cur.push('\n');
        }
        cur.push_str(t);
    }
    if !cur.is_empty() {
        blocks.push(cur.trim().to_string());
    }

    // If nothing looked like multi-line blocks, fall back to per non-empty line.
    if blocks.len() == 1 && !blocks[0].contains('\n') && trimmed.lines().filter(|l| !l.trim().is_empty()).count() > 1 {
        blocks = trimmed
            .lines()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .filter(|t| {
                let lower = t.to_ascii_lowercase();
                lower != "无"
                    && lower != "none"
                    && lower != "n/a"
                    && lower != "na"
                    && !lower.starts_with("# ")
                    && lower != "# issues"
                    && lower != "## issues"
            })
            .map(|s| s.to_string())
            .collect();
    }

    let mut out = Vec::new();
    for (i, block) in blocks.into_iter().enumerate() {
        let lower = block.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "无" | "none" | "n/a" | "na" | "no issues" | "no issue"
        ) || lower.starts_with("# issues")
            || lower == "## residual"
            || lower == "## blocking"
        {
            // Section headers alone are not issues; content under them is.
            if block.lines().count() <= 1 && (lower.starts_with('#') || lower.starts_with("##")) {
                continue;
            }
        }
        let severity = parse_severity_token(&block).unwrap_or(IssueSeverity::Blocking);
        let id = extract_kv(&block, "id")
            .or_else(|| {
                block
                    .lines()
                    .next()
                    .and_then(|l| {
                        let t = l.trim().trim_start_matches('-').trim_start_matches('*').trim();
                        if t.starts_with('I') && t.contains('-') {
                            Some(t.split_whitespace().next().unwrap_or(t).to_string())
                        } else {
                            None
                        }
                    })
            })
            .unwrap_or_else(|| format!("I-{}", i + 1));
        let plan_ref = extract_kv(&block, "plan_ref").unwrap_or_default();
        let path = extract_kv(&block, "path")
            .or_else(|| extract_kv(&block, "file"))
            .unwrap_or_else(|| "n/a".into());
        let symptom = extract_kv(&block, "symptom").unwrap_or_else(|| {
            block
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect()
        });
        let fix_wp = extract_kv(&block, "fix_wp")
            .or_else(|| extract_kv(&block, "suggestion"))
            .unwrap_or_else(|| format!("Fix {id}: {symptom}"));
        out.push(ParsedIssue {
            id,
            severity,
            plan_ref,
            path,
            symptom,
            fix_wp,
            raw: block,
        });
    }
    out
}

fn parse_severity_token(block: &str) -> Option<IssueSeverity> {
    let lower = block.to_ascii_lowercase();
    // severity=… or severity: … (trim so "severity: residual" works)
    for key in ["severity=", "severity:"] {
        if let Some(idx) = lower.find(key) {
            let rest = lower[idx + key.len()..].trim_start();
            let token = rest
                .split(|c: char| c.is_whitespace() || c == ',' || c == '|' || c == ';')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches(|c| c == '`' || c == '*' || c == '"' || c == '\'');
            if token.is_empty() {
                continue;
            }
            return Some(match token {
                "blocking" | "block" | "p0" => IssueSeverity::Blocking,
                "map" | "geb" => IssueSeverity::Map,
                "residual" | "non-blocking" | "nonblocking" | "optional" => {
                    IssueSeverity::Residual
                }
                "out-of-scope" | "outofscope" | "oos" => IssueSeverity::OutOfScope,
                _ => IssueSeverity::Blocking,
            });
        }
    }
    // Chinese / informal (whole-block hints only when no explicit severity=)
    if lower.contains("地图") || lower.contains("geb 指针") || lower.contains("l1/l2") {
        return Some(IssueSeverity::Map);
    }
    if lower.contains("residual") || lower.contains("不阻塞") || lower.contains("可选残留") {
        return Some(IssueSeverity::Residual);
    }
    if lower.contains("范围外") || lower.contains("out of scope") {
        return Some(IssueSeverity::OutOfScope);
    }
    None
}

fn extract_kv(block: &str, key: &str) -> Option<String> {
    let lower_key = key.to_ascii_lowercase();
    for line in block.lines() {
        let t = line.trim().trim_start_matches('-').trim_start_matches('*').trim();
        let lower = t.to_ascii_lowercase();
        for sep in [": ", "=", "："] {
            let pat = format!("{lower_key}{sep}");
            if let Some(rest) = lower.strip_prefix(&pat) {
                // Use original slice with same byte length prefix — prefer after first sep on line.
                if let Some(pos) = t.to_ascii_lowercase().find(sep) {
                    let val = t[pos + sep.len()..].trim();
                    if !val.is_empty() {
                        return Some(val.chars().take(300).collect());
                    }
                }
                let _ = rest;
            }
            // also allow `severity=blocking plan_ref=S5` mid-line
            if let Some(idx) = lower.find(&format!("{lower_key}{sep}")) {
                let after = &t[idx + lower_key.len() + sep.len()..];
                let val = after
                    .split_whitespace()
                    .next()
                    .unwrap_or(after)
                    .trim()
                    .trim_end_matches(',')
                    .to_string();
                if !val.is_empty() {
                    return Some(val.chars().take(300).collect());
                }
            }
        }
    }
    None
}

/// Count ISSUES that block plan-loop success (blocking + map).
pub fn count_blocking_issues(issues: &[ParsedIssue]) -> usize {
    issues
        .iter()
        .filter(|i| i.severity.is_blocking_for_gate())
        .count()
}

/// Read + parse ISSUES; empty if no file / none.
pub fn load_parsed_inspect_issues(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> Vec<ParsedIssue> {
    match read_inspect_issues_text(task, work_dir, project_root) {
        Some(text) => parse_issues_text(&text),
        None => vec![],
    }
}

/// Read ISSUES product into short consumable lines (for Open risks / rework hook).
/// Stable format: each risk line is `ISSUES[<task_id>]: severity=… <snippet>`.
pub fn collect_inspect_issues(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> Vec<String> {
    let parsed = load_parsed_inspect_issues(task, work_dir, project_root);
    if !parsed.is_empty() {
        return parsed
            .into_iter()
            .take(12)
            .map(|i| {
                let snippet: String = i.raw.lines().next().unwrap_or(&i.symptom).chars().take(180).collect();
                format!(
                    "ISSUES[{}]: severity={} plan_ref={} {}",
                    task.id,
                    i.severity.as_str(),
                    if i.plan_ref.is_empty() { "n/a" } else { &i.plan_ref },
                    snippet
                )
            })
            .collect();
    }
    // Fallback: raw lines when parse yielded nothing but file exists.
    let mut lines = Vec::new();
    if let Some(text) = read_inspect_issues_text(task, work_dir, project_root) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return lines;
        }
        let mut n = 0usize;
        for line in trimmed.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            let lower = t.to_ascii_lowercase();
            if lower == "无" || lower == "none" || lower == "n/a" || lower == "na" {
                continue;
            }
            let snippet: String = t.chars().take(200).collect();
            lines.push(format!("ISSUES[{}]: {snippet}", task.id));
            n += 1;
            if n >= 12 {
                break;
            }
        }
        if lines.is_empty() {
            lines.push(format!(
                "ISSUES[{}]: (file present, no actionable items) {}",
                task.id, INSPECT_ISSUES_REL
            ));
        }
    }
    lines
}

/// Lightweight rework-hook note (P2-3 + P-loop): ledger breadcrumb with fix_wp hints.
pub fn rework_placeholder_note(task_id: &str, issues: &[String]) -> String {
    if issues.is_empty() {
        format!(
            "REWORK_HOOK: inspect task `{task_id}` VERDICT=FAIL; no ISSUES body — open `.cco-out/inspect/` and start a rework wave (desktop「回补并再巡检」 or services::start_rework_from_run)"
        )
    } else {
        format!(
            "REWORK_HOOK: inspect task `{task_id}` — {} ISSUE line(s); generate rework TaskIR via start_rework_from_run (max {REWORK_MAX_ROUNDS} rounds); host does not auto-merge/PR",
            issues.len()
        )
    }
}

/// Whether this task should run VERDICT gate after Done (role=inspect or declared VERDICT output).
pub fn task_has_verdict_gate(task: &TaskIR) -> bool {
    task.role == Some(TaskRole::Inspect)
        || task.outputs.iter().any(|o| looks_like_verdict_path(o))
}

/// True when PASS is invalid because blocking/map ISSUES remain (P-loop R-inspect).
pub fn inspect_pass_blocked_by_issues(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
) -> (bool, usize) {
    let parsed = load_parsed_inspect_issues(task, work_dir, project_root);
    let n = count_blocking_issues(&parsed);
    (n > 0, n)
}

/// Count prior rework waves recorded under project `.cco-out/rework/` or handoff timeline.
pub fn count_rework_rounds(project_root: &Path, run_dir: &Path) -> u32 {
    let rework_dir = project_root.join(".cco-out/rework");
    let mut n = 0u32;
    if rework_dir.is_dir() {
        if let Ok(rd) = std::fs::read_dir(&rework_dir) {
            for ent in rd.flatten() {
                let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
                if name.starts_with("round") && (name.ends_with(".md") || name.ends_with(".json")) {
                    n += 1;
                }
            }
        }
    }
    if n == 0 {
        // Fallback: handoff timeline markers
        if let Ok(h) = Handoff::load(run_dir) {
            n = h
                .timeline
                .iter()
                .filter(|l| l.contains("rework_wave") || l.contains("REWORK_WAVE"))
                .count() as u32;
        }
    }
    n
}

/// Build a sequential rework PlanIR: one implement (or map-scoped) task + terminal inspect.
///
/// Does **not** attach rework as downstream of inspect in the original DAG (inspect stays terminal);
/// starts a **new run** wave that pastes ISSUES + plan_ref (R-rework-2).
pub fn build_rework_plan(
    base: &PlanIR,
    issues: &[ParsedIssue],
    round: u32,
    source_run_id: &str,
) -> Result<PlanIR> {
    if issues.is_empty() {
        bail!("no ISSUES to rework");
    }
    let blocking: Vec<&ParsedIssue> = issues
        .iter()
        .filter(|i| i.severity.is_blocking_for_gate())
        .collect();
    let target: Vec<&ParsedIssue> = if blocking.is_empty() {
        // Still allow rework of residual if user explicitly requested (rare).
        issues.iter().collect()
    } else {
        blocking
    };

    let only_map = target.iter().all(|i| i.severity == IssueSeverity::Map);
    let provider = base.default_provider.clone();
    let mode = base.default_mode.clone();
    let opts = base
        .tasks
        .first()
        .map(|t| t.provider_opts.clone())
        .unwrap_or_else(|| serde_json::json!({}));

    let mut issues_body = String::new();
    for i in &target {
        issues_body.push_str(&format!(
            "### {}\n- severity: {}\n- plan_ref: {}\n- path: {}\n- symptom: {}\n- fix_wp: {}\n\n```\n{}\n```\n\n",
            i.id,
            i.severity.as_str(),
            i.plan_ref,
            i.path,
            i.symptom,
            i.fix_wp,
            i.raw
        ));
    }

    let scope_paths: Vec<String> = if only_map {
        MAP_REWORK_PATH_WHITELIST
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        let mut paths: Vec<String> = vec![
            ".cco-out/progress/**".into(),
            ".cco-out/rework/**".into(),
        ];
        for i in &target {
            if i.path != "n/a" && !i.path.is_empty() {
                paths.push(i.path.clone());
            }
        }
        // Broad implement fallback when paths unknown — worker still bound by prompt.
        if paths.len() <= 2 {
            paths.push("**".into());
        }
        paths
    };

    let rework_id = format!("rework-r{round}");
    let inspect_id = format!("reinspect-r{round}");
    let title = if only_map {
        format!("回补地图指针（第 {round} 轮）")
    } else {
        format!("回补阻塞遗漏（第 {round} 轮）")
    };

    let rework_prompt = format!(
        "你是回补实现者（rework wave），不是检验员。\n\
         来源 run: {source_run_id}\n\
         轮次: {round}/{REWORK_MAX_ROUNDS}\n\n\
         ## 必须粘贴的 ISSUES 原文（禁止空话「再检查一下」）\n\
         {issues_body}\n\
         ## 任务\n\
         1. 按每条 fix_wp / plan_ref 修改代码或允许的文档路径。\n\
         2. map 类仅改 GEB/文档指针（CLAUDE.md、docs/CLAUDE.md、总账/本计划勾选行）。\n\
         3. 每完成一条在 `.cco-out/progress/SUMMARY.md` 追加：`plan_ref → 证据`。\n\
         4. 写 `.cco-out/rework/ROUND-{round}.md`：改了什么、对应 ISSUE id。\n\
         5. 不要扩大范围；非目标不实现。\n\n\
         全部完成后最后一行：CCO_DONE ok\n"
    );

    let inspect_prompt = format!(
        "你是检验员（inspect），二次巡检（回补后）。\n\
         对照上轮 ISSUES 与计划勾选，只验下列项是否已清：\n\
         {issues_body}\n\
         ## 必做\n\
         1. 写出计划勾选对照表（plan_ref | PASS|FAIL|SKIP|DEGRADED | 证据）。\n\
         2. 写入 `.cco-out/inspect/VERDICT.md`：首行 **Result: PASS** 或 **Result: FAIL**。\n\
         3. 写入 `.cco-out/inspect/ISSUES.md`：每条含 severity=blocking|map|residual|out-of-scope、plan_ref、path、symptom、fix_wp。\n\
         4. **禁止**在存在未处理 blocking/map 时写 PASS。\n\
         5. residual 可附录；不得伪装成「没问题」。\n\
         6. 默认不改业务代码；只写 `.cco-out/inspect/**`。\n\n\
         最后一行：CCO_DONE ok\n"
    );

    let rework_task = TaskIR {
        id: rework_id.clone(),
        title,
        depends_on: vec![],
        group: Some(format!("rework-{round}")),
        provider: provider.clone(),
        mode: mode.clone(),
        prompt: rework_prompt,
        acceptance: None,
        timeout_secs: None,
        worktree: Some(false),
        provider_opts: opts.clone(),
        optional: false,
        include: true,
        role: Some(TaskRole::Implement),
        scope: Some(crate::plan::TaskScope {
            paths: scope_paths,
            readonly: vec![],
            forbid: if only_map {
                vec!["src/**".into(), "web/**".into(), "src-tauri/**".into()]
            } else {
                vec![]
            },
        }),
        outputs: vec![
            format!(".cco-out/rework/ROUND-{round}.md"),
            ".cco-out/progress/SUMMARY.md".into(),
        ],
    };

    let inspect_task = TaskIR {
        id: inspect_id,
        title: format!("回补后巡检（第 {round} 轮）"),
        depends_on: vec![rework_id],
        group: Some(format!("rework-{round}")),
        provider: provider.clone(),
        mode: mode.clone(),
        prompt: inspect_prompt,
        acceptance: None,
        timeout_secs: None,
        worktree: Some(false),
        provider_opts: opts,
        optional: false,
        include: true,
        role: Some(TaskRole::Inspect),
        scope: Some(crate::plan::TaskScope {
            paths: vec![INSPECT_VERDICT_REL.into(), ".cco-out/inspect/**".into()],
            readonly: vec!["**".into()],
            forbid: vec![],
        }),
        outputs: vec![
            INSPECT_VERDICT_REL.into(),
            INSPECT_ISSUES_REL.into(),
        ],
    };

    let mut ir = PlanIR {
        schema: "cco-plan/v1".into(),
        name: format!("{}-rework-r{round}", base.name),
        adapter: "rework-wave".into(),
        source_path: base.source_path.clone(),
        max_parallel: 1,
        on_failure: base.on_failure,
        retry_max: 0,
        default_provider: provider,
        default_mode: mode,
        worktree: base.worktree,
        require_inspect: true,
        tasks: vec![rework_task, inspect_task],
    };
    crate::plan::materialize_role_defaults(&mut ir);
    ir.validate()?;
    Ok(ir)
}

/// Append ACCEPTED_RESIDUAL note to handoff open_risks (P-loop Q7). Does not flip run status.
pub fn accept_residual_on_handoff(
    plan: &PlanIR,
    state: &RunState,
    note: &str,
) -> Result<()> {
    let mut h = load_or_init(plan, state)?;
    h.updated = Utc::now();
    let line = if note.trim().is_empty() {
        format!(
            "ACCEPTED_RESIDUAL: user accepted remaining open risks at {}",
            Utc::now().to_rfc3339()
        )
    } else {
        format!(
            "ACCEPTED_RESIDUAL: {} ({})",
            note.trim().chars().take(300).collect::<String>(),
            Utc::now().to_rfc3339()
        )
    };
    if !h.open_risks.iter().any(|r| r.starts_with("ACCEPTED_RESIDUAL:")) {
        h.open_risks.push(line.clone());
    } else {
        // refresh note
        h.open_risks.retain(|r| !r.starts_with("ACCEPTED_RESIDUAL:"));
        h.open_risks.push(line.clone());
    }
    h.push_timeline(format!(
        "{} · accepted_residual · {}",
        Utc::now().to_rfc3339(),
        note.chars().take(80).collect::<String>()
    ));
    h.instructions_for_next = format!(
        "- {line}\n- blocking items were explicitly accepted; do not treat as pure PASS\n"
    );
    h.save(&state.run_dir)
}

/// Snapshot for desktop / live view (P-loop L2).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InspectLoopView {
    pub verdict: Option<String>,
    pub blocking_count: usize,
    pub residual_count: usize,
    pub issue_preview: Vec<String>,
    pub can_rework: bool,
    pub rework_round: u32,
    pub rework_max: u32,
    pub accepted_residual: bool,
    pub require_inspect: bool,
}

/// Build inspect-loop summary from project inspect products + handoff.
pub fn inspect_loop_view(
    plan: Option<&PlanIR>,
    state: &RunState,
    project_root: &Path,
) -> InspectLoopView {
    let require_inspect = plan.map(|p| p.require_inspect).unwrap_or(false);
    let rework_round = count_rework_rounds(project_root, &state.run_dir);
    let mut view = InspectLoopView {
        require_inspect,
        rework_round,
        rework_max: REWORK_MAX_ROUNDS,
        ..Default::default()
    };

    // Prefer role=inspect task; else conventional paths.
    let inspect_task = plan.and_then(|p| {
        p.tasks
            .iter()
            .rev()
            .find(|t| t.role == Some(TaskRole::Inspect))
    });

    let work_dir = state.project_root.as_path();
    let verdict = if let Some(t) = inspect_task {
        read_inspect_verdict(t, work_dir, project_root)
    } else {
        // Conventional path only
        let path = project_root.join(INSPECT_VERDICT_REL);
        if path.is_file() {
            std::fs::read_to_string(&path)
                .map(|t| parse_verdict_text(&t))
                .unwrap_or(InspectVerdict::Unknown)
        } else {
            InspectVerdict::Unknown
        }
    };
    view.verdict = match verdict {
        InspectVerdict::Pass => Some("PASS".into()),
        InspectVerdict::Fail => Some("FAIL".into()),
        InspectVerdict::Unknown => {
            if project_root.join(INSPECT_VERDICT_REL).is_file() {
                Some("UNKNOWN".into())
            } else {
                None
            }
        }
    };

    let parsed = if let Some(t) = inspect_task {
        load_parsed_inspect_issues(t, work_dir, project_root)
    } else {
        let path = project_root.join(INSPECT_ISSUES_REL);
        if path.is_file() {
            std::fs::read_to_string(&path)
                .map(|t| parse_issues_text(&t))
                .unwrap_or_default()
        } else {
            vec![]
        }
    };
    view.blocking_count = count_blocking_issues(&parsed);
    view.residual_count = parsed
        .iter()
        .filter(|i| {
            matches!(
                i.severity,
                IssueSeverity::Residual | IssueSeverity::OutOfScope
            )
        })
        .count();
    view.issue_preview = parsed
        .iter()
        .take(8)
        .map(|i| {
            format!(
                "{} severity={} {}",
                i.id,
                i.severity.as_str(),
                i.symptom.chars().take(100).collect::<String>()
            )
        })
        .collect();

    if let Ok(h) = Handoff::load(&state.run_dir) {
        view.accepted_residual = h
            .open_risks
            .iter()
            .any(|r| r.starts_with("ACCEPTED_RESIDUAL:"));
        if view.issue_preview.is_empty() {
            view.issue_preview = h
                .open_risks
                .iter()
                .filter(|r| r.contains("ISSUES[") || r.contains("REWORK_HOOK"))
                .take(6)
                .cloned()
                .collect();
        }
    }

    let needs_rework = matches!(verdict, InspectVerdict::Fail)
        || view.blocking_count > 0
        || view.verdict.as_deref() == Some("UNKNOWN") && require_inspect;
    view.can_rework = needs_rework
        && !view.accepted_residual
        && rework_round < REWORK_MAX_ROUNDS
        && matches!(
            state.status,
            RunStatus::Paused | RunStatus::Failed | RunStatus::Completed | RunStatus::Aborted
        );

    view
}

/// Create empty handoff shell and write to disk.
pub fn write_shell(plan: &PlanIR, state: &RunState) -> Result<()> {
    let h = Handoff::init_shell(plan, state);
    h.save(&state.run_dir)
}

/// Board → running on task start.
pub fn on_task_start(plan: &PlanIR, state: &RunState, task_id: &str) -> Result<()> {
    let mut h = load_or_init(plan, state)?;
    h.updated = Utc::now();
    h.status = "running".into();
    h.set_board_status(task_id, "running", None, "");
    h.push_timeline(format!(
        "{} · task_start · {task_id}",
        Utc::now().to_rfc3339()
    ));
    let done: Vec<String> = h
        .board
        .iter()
        .filter(|r| r.status == "done" || r.status == "skipped")
        .map(|r| r.id.clone())
        .collect();
    h.instructions_for_next = default_next_instructions(plan, &done);
    h.save(&state.run_dir)
}

/// Merge fragment after task terminal; update Board / Timeline / Open risks.
pub fn on_task_end(
    plan: &PlanIR,
    state: &RunState,
    task: &TaskIR,
    result: &TaskResult,
    work_dir: Option<&Path>,
) -> Result<()> {
    let mut h = load_or_init(plan, state)?;
    h.updated = Utc::now();

    let st_label = status_label(result.status);
    let cost = result.cost_usd;
    let notes = result
        .error
        .as_deref()
        .map(|e| e.chars().take(120).collect::<String>())
        .unwrap_or_default();

    h.set_board_status(&task.id, st_label, cost, &notes);
    h.push_timeline(format!(
        "{} · task_end · {} · {st_label}",
        Utc::now().to_rfc3339(),
        task.id
    ));

    let wd = work_dir
        .map(|p| p.to_path_buf())
        .or_else(|| {
            state
                .tasks
                .get(&task.id)
                .and_then(|t| t.work_dir.clone())
        })
        .unwrap_or_else(|| state.project_root.clone());

    let branch = state
        .tasks
        .get(&task.id)
        .and_then(|t| t.worktree_branch.clone());

    let mut artifacts = Vec::new();
    for o in &task.outputs {
        let path = resolve_output_path(o, &wd, &state.project_root);
        if path.exists() {
            artifacts.push(o.clone());
        }
    }

    let summary = extract_summary(task, &wd, &state.project_root, result);

    let mut risks = Vec::new();
    if result.status != TaskStatus::Done {
        if let Some(err) = &result.error {
            risks.push(format!("{}: {err}", task.id));
        } else {
            risks.push(format!("{} ended as {st_label}", task.id));
        }
    }

    // P2-3: on VERDICT=FAIL (or error mentions it), fold ISSUES into fragment risks + Open risks.
    let verdict_fail = result
        .error
        .as_deref()
        .map(|e| e.contains("VERDICT=FAIL") || e.contains("inspect VERDICT"))
        .unwrap_or(false)
        || (task_has_verdict_gate(task)
            && read_inspect_verdict(task, &wd, &state.project_root) == InspectVerdict::Fail);
    let mut rework_note: Option<String> = None;
    if verdict_fail {
        let issues = collect_inspect_issues(task, &wd, &state.project_root);
        if issues.is_empty() {
            // Still leave a stable ISSUES clue even if file missing.
            risks.push(format!(
                "ISSUES[{}]: VERDICT=FAIL — see {} (missing or empty)",
                task.id, INSPECT_ISSUES_REL
            ));
        } else {
            for line in &issues {
                if !risks.iter().any(|r| r == line) {
                    risks.push(line.clone());
                }
            }
        }
        let note = rework_placeholder_note(&task.id, &issues);
        risks.push(note.clone());
        rework_note = Some(note);
        h.push_timeline(format!(
            "{} · inspect_verdict_fail · {} · ISSUES folded",
            Utc::now().to_rfc3339(),
            task.id
        ));
    }

    h.fragments.insert(
        task.id.clone(),
        Fragment {
            status: st_label.into(),
            provider: task.provider.clone(),
            work_dir: Some(wd.display().to_string()),
            branch,
            summary,
            artifacts,
            risks: risks.clone(),
        },
    );

    // Rebuild open risks from all fragments + current
    h.open_risks = h
        .fragments
        .values()
        .flat_map(|f| f.risks.iter().cloned())
        .collect();

    let done: Vec<String> = h
        .board
        .iter()
        .filter(|r| r.status == "done" || r.status == "skipped")
        .map(|r| r.id.clone())
        .collect();
    let mut next = default_next_instructions(plan, &done);
    if let Some(note) = rework_note {
        // Stable rework hook surface for humans / next wave (not auto-scheduled).
        next = format!(
            "- {note}\n- consumable ISSUES lines are under Open risks (prefix ISSUES[{}])\n{next}",
            task.id
        );
    }
    h.instructions_for_next = next;

    h.save(&state.run_dir)
}

/// Final run status stamp on handoff.
pub fn on_run_end(plan: &PlanIR, state: &RunState, status: RunStatus) -> Result<()> {
    let mut h = load_or_init(plan, state)?;
    h.updated = Utc::now();
    h.status = match status {
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::Paused => "paused",
        RunStatus::Aborted => "aborted",
        RunStatus::Running => "running",
        RunStatus::Validated => "validated",
        RunStatus::Init => "init",
    }
    .into();
    h.push_timeline(format!(
        "{} · run_end · {}",
        Utc::now().to_rfc3339(),
        h.status
    ));
    h.save(&state.run_dir)
}

fn load_or_init(plan: &PlanIR, state: &RunState) -> Result<Handoff> {
    let path = Handoff::path_json(&state.run_dir);
    if path.exists() {
        Handoff::load(&state.run_dir)
    } else {
        Ok(Handoff::init_shell(plan, state))
    }
}

// ── P1-5: prompt prefix injection ────────────────────────────────────────

/// Build the `[CCO_HANDOFF]…[/CCO_HANDOFF]` block for a task about to start.
///
/// Short summary only: identity + scope + outputs + Board table + depends_on
/// Fragments. Missing handoff file → empty Board/Fragments shell (no panic).
pub fn build_prompt_prefix(task: &TaskIR, run_dir: &Path) -> String {
    let role = role_str(task.role).unwrap_or_else(|| "-".into());
    let (paths, forbid) = match &task.scope {
        Some(s) => (s.paths.join(", "), s.forbid.join(", ")),
        None => (String::new(), String::new()),
    };
    let paths = if paths.is_empty() { "-".into() } else { paths };
    let forbid = if forbid.is_empty() { "-".into() } else { forbid };
    let deps = if task.depends_on.is_empty() {
        "-".into()
    } else {
        task.depends_on.join(", ")
    };
    let outputs = if task.outputs.is_empty() {
        "-".into()
    } else {
        task.outputs.join(", ")
    };
    let ledger = Handoff::path_md(run_dir).display().to_string();

    let handoff = if Handoff::path_json(run_dir).exists() {
        Handoff::load(run_dir).ok()
    } else {
        None
    };

    let mut body = String::new();
    body.push_str(HANDOFF_PROMPT_OPEN);
    body.push('\n');
    body.push_str(&format!(
        "你是 task={} provider={} role={}\n",
        task.id, task.provider, role
    ));
    body.push_str(&format!("scope.paths={paths}\n"));
    body.push_str(&format!("scope.forbid={forbid}\n"));
    body.push_str(&format!("必读: Board + Fragments(depends_on: {deps})\n"));
    body.push_str(&format!("全局账本: {ledger}\n"));
    body.push_str(&format!("你的 outputs: {outputs}\n"));
    body.push_str("完成后最后一行: CCO_DONE ok\n");

    // Short Board table (status snapshot only).
    body.push_str("\n## Board\n");
    body.push_str("| id | provider | role | status | scope | outputs | notes |\n");
    body.push_str("|----|----------|------|--------|-------|---------|-------|\n");
    if let Some(h) = &handoff {
        if h.board.is_empty() {
            body.push_str("| - | - | - | - | - | - | (empty) |\n");
        } else {
            for r in &h.board {
                let r_role = r.role.as_deref().unwrap_or("-");
                let outs = if r.outputs.is_empty() {
                    "-".into()
                } else {
                    r.outputs.join(", ")
                };
                let scope = if r.scope.is_empty() {
                    "-"
                } else {
                    r.scope.as_str()
                };
                let notes: String = if r.notes.is_empty() {
                    "-".into()
                } else {
                    r.notes.chars().take(80).collect()
                };
                body.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} | {} |\n",
                    r.id, r.provider, r_role, r.status, scope, outs, notes
                ));
            }
        }
    } else {
        body.push_str("| - | - | - | - | - | - | (no handoff yet) |\n");
    }

    // Only depends_on Fragments (not full ledger).
    body.push_str("\n## Fragments (depends_on)\n");
    if task.depends_on.is_empty() {
        body.push_str("_none_\n");
    } else if let Some(h) = &handoff {
        let mut any = false;
        for dep in &task.depends_on {
            if let Some(f) = h.fragments.get(dep) {
                any = true;
                body.push_str(&format!("### {dep}\n"));
                body.push_str(&format!(
                    "- status: {} · provider: {}\n",
                    f.status, f.provider
                ));
                if !f.summary.is_empty() {
                    let s: String = f.summary.chars().take(PREFIX_SUMMARY_CHARS).collect();
                    body.push_str(&format!("- summary: {s}\n"));
                }
                if !f.artifacts.is_empty() {
                    body.push_str(&format!("- artifacts: {}\n", f.artifacts.join(", ")));
                }
                if !f.risks.is_empty() {
                    body.push_str(&format!("- risks: {}\n", f.risks.join("; ")));
                }
            } else {
                body.push_str(&format!("### {dep}\n- (no fragment yet)\n"));
                any = true;
            }
        }
        if !any {
            body.push_str("_none_\n");
        }
    } else {
        for dep in &task.depends_on {
            body.push_str(&format!("### {dep}\n- (no handoff yet)\n"));
        }
    }

    body.push_str(HANDOFF_PROMPT_CLOSE);
    body.push('\n');
    body
}

/// Prepend handoff summary to the business prompt. Idempotent if already wrapped.
///
/// On missing/corrupt handoff: still inject identity shell (never panics).
pub fn with_handoff_prefix(prompt: &str, task: &TaskIR, run_dir: &Path) -> String {
    if prompt.contains(HANDOFF_PROMPT_OPEN) {
        return prompt.to_string();
    }
    let prefix = build_prompt_prefix(task, run_dir);
    if prompt.trim().is_empty() {
        prefix
    } else {
        format!("{prefix}\n{prompt}")
    }
}

fn extract_summary(
    task: &TaskIR,
    work_dir: &Path,
    project_root: &Path,
    result: &TaskResult,
) -> String {
    // Prefer declared outputs that look like summary / md
    for o in &task.outputs {
        let lower = o.to_ascii_lowercase();
        if lower.contains("summary") || lower.ends_with(".md") {
            let path = resolve_output_path(o, work_dir, project_root);
            if let Ok(text) = std::fs::read_to_string(&path) {
                let s: String = text.chars().take(400).collect();
                if !s.trim().is_empty() {
                    return s.trim().replace('\n', " ");
                }
            }
        }
    }
    // Fallback: result.raw.result string
    if let Some(s) = result
        .raw
        .get("result")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(200).collect::<String>())
    {
        if !s.is_empty() {
            return s;
        }
    }
    if let Some(err) = &result.error {
        return err.chars().take(200).collect();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{OnFailure, PlanIR, TaskIR};
    use crate::state::RunState;
    use tempfile::tempdir;

    fn sample_plan(outputs_a: Vec<String>) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "t".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("plan.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![
                TaskIR {
                    id: "a".into(),
                    title: "a".into(),
                    depends_on: vec![],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "do a".into(),
                    acceptance: None,
                    timeout_secs: None,
                    worktree: None,
                    provider_opts: serde_json::json!({}),
                    optional: false,
                    include: true,
                    role: None,
                    scope: None,
                    outputs: outputs_a,
                },
                TaskIR {
                    id: "b".into(),
                    title: "b".into(),
                    depends_on: vec!["a".into()],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "do b".into(),
                    acceptance: None,
                    timeout_secs: None,
                    worktree: None,
                    provider_opts: serde_json::json!({}),
                    optional: false,
                    include: true,
                    role: None,
                    scope: None,
                    outputs: vec![],
                },
            ],
        }
    }

    #[test]
    fn shell_and_task_lifecycle() {
        let tmp = tempdir().unwrap();
        let run_dir = tmp.path().join("run1");
        std::fs::create_dir_all(&run_dir).unwrap();
        let plan = sample_plan(vec![".cco-out/a/SUMMARY.md".into()]);
        let state = RunState::new(
            "run1".into(),
            tmp.path().to_path_buf(),
            &plan,
            run_dir.clone(),
        );

        write_shell(&plan, &state).unwrap();
        assert!(Handoff::path_md(&run_dir).exists());
        assert!(Handoff::path_json(&run_dir).exists());
        let h = Handoff::load(&run_dir).unwrap();
        assert_eq!(h.board.len(), 2);
        assert!(h.board.iter().all(|r| r.status == "pending"));

        on_task_start(&plan, &state, "a").unwrap();
        let h = Handoff::load(&run_dir).unwrap();
        assert_eq!(h.board.iter().find(|r| r.id == "a").unwrap().status, "running");

        let out = tmp.path().join(".cco-out/a");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("SUMMARY.md"), "did a\n").unwrap();

        let result = TaskResult {
            status: TaskStatus::Done,
            exit_code: Some(0),
            stdout_path: None,
            session_id: None,
            agent_id: None,
            cost_usd: Some(0.01),
            raw: serde_json::json!({"result": "fake ok"}),
            error: None,
        };
        on_task_end(&plan, &state, &plan.tasks[0], &result, Some(tmp.path())).unwrap();
        let h = Handoff::load(&run_dir).unwrap();
        assert_eq!(h.board.iter().find(|r| r.id == "a").unwrap().status, "done");
        assert!(h.fragments.contains_key("a"));
        assert!(h.fragments["a"].summary.contains("did a") || !h.fragments["a"].summary.is_empty());
        let md = std::fs::read_to_string(Handoff::path_md(&run_dir)).unwrap();
        assert!(md.contains("## Board"));
        assert!(md.contains("## Fragments"));
        assert!(md.contains("## Open risks"));
        assert!(md.contains("## Instructions for next worker"));
    }

    #[test]
    fn missing_outputs_detected() {
        let tmp = tempdir().unwrap();
        let plan = sample_plan(vec![".cco-out/missing.md".into()]);
        let missing = missing_outputs(&plan.tasks[0], tmp.path(), tmp.path());
        assert_eq!(missing, vec![".cco-out/missing.md".to_string()]);
    }

    /// P1-5: missing handoff file → identity shell, no panic.
    #[test]
    fn prompt_prefix_without_handoff_file() {
        let tmp = tempdir().unwrap();
        let plan = sample_plan(vec![]);
        let task = &plan.tasks[0];
        let prefix = build_prompt_prefix(task, tmp.path());
        assert!(prefix.contains(HANDOFF_PROMPT_OPEN));
        assert!(prefix.contains(HANDOFF_PROMPT_CLOSE));
        assert!(prefix.contains("task=a"));
        assert!(prefix.contains("provider=fake"));
        assert!(prefix.contains("CCO_DONE ok"));
        assert!(prefix.contains("(no handoff yet)") || prefix.contains("## Board"));
        let wrapped = with_handoff_prefix("do a\nCCO_DONE ok", task, tmp.path());
        assert!(wrapped.starts_with(HANDOFF_PROMPT_OPEN));
        assert!(wrapped.contains("do a"));
        // idempotent
        let twice = with_handoff_prefix(&wrapped, task, tmp.path());
        assert_eq!(twice.matches(HANDOFF_PROMPT_OPEN).count(), 1);
    }

    /// P1-5: after task a ends, task b prefix includes Board + fragment a.
    #[test]
    fn prompt_prefix_includes_depends_on_fragment() {
        let tmp = tempdir().unwrap();
        let run_dir = tmp.path().join("run1");
        std::fs::create_dir_all(&run_dir).unwrap();
        let plan = sample_plan(vec![".cco-out/a/SUMMARY.md".into()]);
        let state = RunState::new(
            "run1".into(),
            tmp.path().to_path_buf(),
            &plan,
            run_dir.clone(),
        );
        write_shell(&plan, &state).unwrap();
        on_task_start(&plan, &state, "a").unwrap();
        let out = tmp.path().join(".cco-out/a");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("SUMMARY.md"), "summary from a\n").unwrap();
        let result = TaskResult {
            status: TaskStatus::Done,
            exit_code: Some(0),
            stdout_path: None,
            session_id: None,
            agent_id: None,
            cost_usd: Some(0.01),
            raw: serde_json::json!({"result": "fake ok"}),
            error: None,
        };
        on_task_end(&plan, &state, &plan.tasks[0], &result, Some(tmp.path())).unwrap();

        let prefix = build_prompt_prefix(&plan.tasks[1], &run_dir);
        assert!(prefix.contains(HANDOFF_PROMPT_OPEN));
        assert!(prefix.contains("task=b"));
        assert!(prefix.contains("depends_on: a"));
        assert!(prefix.contains("## Board"));
        assert!(prefix.contains("| a |"));
        assert!(prefix.contains("### a"));
        assert!(
            prefix.contains("summary from a") || prefix.contains("fake ok"),
            "prefix should include dep fragment summary: {prefix}"
        );
        assert!(prefix.contains(HANDOFF_PROMPT_CLOSE));
        let full = with_handoff_prefix("do b\nCCO_DONE ok", &plan.tasks[1], &run_dir);
        assert!(full.contains("do b"));
    }

    // ── P2-3 unit tests ──────────────────────────────────────────────────

    #[test]
    fn parse_verdict_fail_and_pass() {
        assert_eq!(parse_verdict_text("FAIL\nreason"), InspectVerdict::Fail);
        assert_eq!(parse_verdict_text("PASS\nok"), InspectVerdict::Pass);
        assert_eq!(
            parse_verdict_text("VERDICT: FAIL — scope leak"),
            InspectVerdict::Fail
        );
        assert_eq!(
            parse_verdict_text("VERDICT=PASS"),
            InspectVerdict::Pass
        );
        assert_eq!(parse_verdict_text("maybe later"), InspectVerdict::Unknown);
        // FAIL wins when both present in body
        assert_eq!(
            parse_verdict_text("notes\nPASS was hoped\nbut VERDICT=FAIL overall"),
            InspectVerdict::Fail
        );
    }

    #[test]
    fn on_task_end_folds_issues_on_verdict_fail() {
        let tmp = tempdir().unwrap();
        let run_dir = tmp.path().join("run-inspect");
        std::fs::create_dir_all(&run_dir).unwrap();
        let inspect_dir = tmp.path().join(".cco-out/inspect");
        std::fs::create_dir_all(&inspect_dir).unwrap();
        std::fs::write(inspect_dir.join("VERDICT.md"), "FAIL\nscope leak in feat-a\n").unwrap();
        std::fs::write(
            inspect_dir.join("ISSUES.md"),
            "- file: examples/demo_a/x.rs\n- symptom: wrote outside scope\n- suggestion: revert + narrow edit\n",
        )
        .unwrap();

        let mut plan = sample_plan(vec![
            ".cco-out/inspect/VERDICT.md".into(),
            ".cco-out/inspect/ISSUES.md".into(),
        ]);
        plan.tasks[0].id = "inspect".into();
        plan.tasks[0].role = Some(TaskRole::Inspect);
        plan.tasks[0].outputs = vec![
            ".cco-out/inspect/VERDICT.md".into(),
            ".cco-out/inspect/ISSUES.md".into(),
        ];

        let state = RunState::new(
            "run-inspect".into(),
            tmp.path().to_path_buf(),
            &plan,
            run_dir.clone(),
        );
        write_shell(&plan, &state).unwrap();

        let result = TaskResult {
            status: TaskStatus::Failed,
            exit_code: Some(0),
            stdout_path: None,
            session_id: None,
            agent_id: None,
            cost_usd: Some(0.02),
            raw: serde_json::json!({}),
            error: Some("inspect VERDICT=FAIL (2 ISSUES line(s) for rework)".into()),
        };
        on_task_end(&plan, &state, &plan.tasks[0], &result, Some(tmp.path())).unwrap();

        let h = Handoff::load(&run_dir).unwrap();
        assert_eq!(
            h.board.iter().find(|r| r.id == "inspect").unwrap().status,
            "failed"
        );
        assert!(
            h.open_risks.iter().any(|r| r.contains("ISSUES[inspect]")),
            "open_risks={:?}",
            h.open_risks
        );
        assert!(
            h.open_risks.iter().any(|r| r.contains("REWORK_HOOK")),
            "expected REWORK_HOOK in open_risks={:?}",
            h.open_risks
        );
        assert!(
            h.instructions_for_next.contains("REWORK_HOOK")
                || h.instructions_for_next.contains("ISSUES"),
            "instructions={}",
            h.instructions_for_next
        );
        let md = std::fs::read_to_string(Handoff::path_md(&run_dir)).unwrap();
        assert!(md.contains("ISSUES[inspect]") || md.contains("REWORK_HOOK"));
    }

    // ── P-loop unit tests ───────────────────────────────────────────────

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
        assert_eq!(count_blocking_issues(&parsed), 2);
    }

    #[test]
    fn parse_issues_fail_closed_without_severity() {
        let parsed = parse_issues_text("- missing plan pointer in CLAUDE.md\n");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].severity, IssueSeverity::Blocking);
    }

    #[test]
    fn parse_verdict_result_prefix() {
        assert_eq!(
            parse_verdict_text("**Result: FAIL**\n\n| plan_ref |"),
            InspectVerdict::Fail
        );
        assert_eq!(
            parse_verdict_text("Result: PASS\nok"),
            InspectVerdict::Pass
        );
    }

    #[test]
    fn build_rework_plan_has_inspect_sink_and_plan_refs() {
        let base = sample_plan(vec![]);
        let issues = vec![
            ParsedIssue {
                id: "I-1".into(),
                severity: IssueSeverity::Map,
                plan_ref: "§8".into(),
                path: "CLAUDE.md".into(),
                symptom: "stale".into(),
                fix_wp: "fix pointer".into(),
                raw: "severity=map plan_ref=§8 path=CLAUDE.md".into(),
            },
            ParsedIssue {
                id: "I-2".into(),
                severity: IssueSeverity::Blocking,
                plan_ref: "S5".into(),
                path: "src/lib.rs".into(),
                symptom: "missing".into(),
                fix_wp: "implement".into(),
                raw: "severity=blocking plan_ref=S5".into(),
            },
        ];
        let ir = build_rework_plan(&base, &issues, 1, "run-src").unwrap();
        assert!(ir.require_inspect);
        assert_eq!(ir.tasks.len(), 2);
        assert_eq!(ir.tasks[0].role, Some(TaskRole::Implement));
        assert_eq!(ir.tasks[1].role, Some(TaskRole::Inspect));
        assert!(ir.tasks[1].depends_on.contains(&ir.tasks[0].id));
        assert!(ir.tasks[0].prompt.contains("I-1") || ir.tasks[0].prompt.contains("severity"));
        assert!(ir.tasks[0].prompt.contains("plan_ref") || ir.tasks[0].prompt.contains("S5"));
        assert!(ir.tasks[1].prompt.contains("禁止") || ir.tasks[1].prompt.contains("blocking"));
        ir.validate().unwrap();
    }

    #[test]
    fn map_only_rework_uses_whitelist_scope() {
        let base = sample_plan(vec![]);
        let issues = vec![ParsedIssue {
            id: "I-map".into(),
            severity: IssueSeverity::Map,
            plan_ref: "GEB".into(),
            path: "CLAUDE.md".into(),
            symptom: "stale".into(),
            fix_wp: "update L1".into(),
            raw: "severity=map".into(),
        }];
        let ir = build_rework_plan(&base, &issues, 1, "r1").unwrap();
        let scope = ir.tasks[0].scope.as_ref().unwrap();
        assert!(
            scope.paths.iter().any(|p| p.contains("CLAUDE") || p.contains("docs")),
            "paths={:?}",
            scope.paths
        );
        assert!(ir.tasks[0].title.contains("地图") || ir.tasks[0].prompt.contains("GEB"));
    }
}
