//! Post-task gates: outputs · inspect VERDICT (handoff/domain API only) · handoff end.
//!
//! VERDICT **text parse stays in domain::inspect** (via handoff facade). This file only
//! isolates the call sites so the orchestrator loop does not grow more gate logic.
//!
//! [INPUT]: TaskIR · work_dir · TaskResult
//! [OUTPUT]: may flip result to Failed
//! [POS]: runtime/scheduler
//! [PROTOCOL]: 禁止在此重写 VERDICT 解析；变更时更新 scheduler/mod.rs

use std::process::Command;
use std::path::Path;

use tracing::warn;

use super::super::provider::TaskResult;
use super::Scheduler;
use crate::plan::TaskIR;
use crate::runtime::handoff;

/// Run `git -C work_dir <args…>`, return trimmed stdout (None on failure).
fn git_output(work_dir: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(work_dir);
    cmd.args(args);
    cmd.output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

impl Scheduler {
    /// P1-4: if TaskIR.outputs non-empty and any missing → Failed.
    pub(super) fn enforce_outputs(
        &self,
        task: &TaskIR,
        work_dir: &std::path::Path,
        result: &mut TaskResult,
    ) {
        if result.status != super::super::provider::TaskStatus::Done {
            return;
        }
        let missing = handoff::missing_outputs(task, work_dir, &self.state.project_root);
        if missing.is_empty() {
            return;
        }
        result.status = super::super::provider::TaskStatus::Failed;
        result.error = Some(format!("missing outputs: {}", missing.join(", ")));
        warn!(
            task = %task.id,
            missing = ?missing,
            "task failed: required outputs missing"
        );
    }

    /// 统一成功裁决（契约层 T1/T3，替代 noop guard）：可写角色 Done 不能只信 exit，
    /// 必须有三类产出证据之一——② 执行动作证据（decoder 判定）∨ ③ outputs 全部存在
    /// ∨ ④ worktree 有产出。① 平台完成信号 与 ⑤ exit 通过是前置门槛（进入此 gate 时
    /// `status==Done` 已隐含 ⑤）。三者全无 → Failed，人话说明缺哪项。
    ///
    /// 与旧 noop guard 的差异：
    /// - `scope=None` 不再早退（可写判定改用 role + provider，不再靠 scope.paths）；
    /// - 无 worktree 模式同样生效（由 ② / ③ 裁决）；
    /// - 产出证据不再只认 worktree diff，还认执行动作证据与 outputs。
    pub(super) fn enforce_success_evidence(
        &self,
        task: &TaskIR,
        work_dir: &std::path::Path,
        result: &mut TaskResult,
    ) {
        use super::super::provider::TaskStatus;
        if result.status != TaskStatus::Done {
            return;
        }
        // fake 是 drill/demo 桩，从不写真实文件 — 恒 Done。
        if task.provider.eq_ignore_ascii_case("fake") {
            return;
        }
        // 只读角色允许零产出（inspect/scout）；integrate 通常只移动产物，不误伤。
        if matches!(
            task.role,
            Some(crate::plan::TaskRole::Inspect)
                | Some(crate::plan::TaskRole::Scout)
                | Some(crate::plan::TaskRole::Integrate)
        ) {
            return;
        }

        let mut missing: Vec<String> = Vec::new();
        let mut has_any = false;

        // ② 执行动作证据（decoder 判定：command_execution / tool / 结果对象 / 非空输出）。
        if result.execution_evidence {
            has_any = true;
        } else {
            missing.push("无执行动作证据".into());
        }

        // ③ outputs 全部存在（声明了才作为产出证据；未声明不算，也不报缺失）。
        if !task.outputs.is_empty() {
            let missing_out = handoff::missing_outputs(task, work_dir, &self.state.project_root);
            if missing_out.is_empty() {
                has_any = true;
            } else {
                missing.push(format!("outputs 缺失: {}", missing_out.join(", ")));
            }
        }

        // ④ worktree 模式：HEAD≠fork_base 或有未提交改动。
        match self.worktree_has_progress(task, work_dir) {
            Some(true) => has_any = true,
            Some(false) => missing.push("worktree 相对 fork 起点无改动".into()),
            None => {} // 非 worktree / 无 fork_base：由②③裁决
        }

        if has_any {
            return;
        }

        result.status = TaskStatus::Failed;
        result.error = Some(format!(
            "任务空转：无执行动作 / 无产物 / 无 outputs（{} 缺：{}）。依赖产物未同步或任务未真正执行，请检查前置步骤。",
            task.id,
            missing.join("；")
        ));
        warn!(
            task = %task.id,
            missing = ?missing,
            "task failed: empty-spin (no execution / no artifact / no outputs)"
        );
    }

    /// ④ worktree 产出证据：HEAD≠fork_base 或有未提交改动。None = 非 worktree / 无记录起点。
    fn worktree_has_progress(&self, task: &TaskIR, work_dir: &std::path::Path) -> Option<bool> {
        if !work_dir.is_dir() {
            return None;
        }
        let wd_meta = self.state.task_dir(&task.id).join("work_dir.json");
        let fork_base_sha: Option<String> = std::fs::read_to_string(&wd_meta)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| {
                v.get("fork_base_sha")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            });
        let Some(base_sha) = fork_base_sha else {
            return None;
        };
        let head = git_output(work_dir, &["rev-parse", "HEAD"]);
        if head != Some(base_sha.clone()) {
            return Some(true);
        }
        let status = git_output(work_dir, &["status", "--porcelain"]);
        let has_changes = status
            .map(|s| s.lines().any(|l| !l.trim().is_empty()))
            .unwrap_or(false);
        Some(has_changes)
    }

    /// P2-3 + P-loop: inspect VERDICT gate via handoff/domain API only (no parse here).
    pub(super) fn enforce_inspect_verdict(
        &self,
        task: &TaskIR,
        work_dir: &std::path::Path,
        result: &mut TaskResult,
    ) {
        use super::super::provider::TaskStatus;
        if result.status != TaskStatus::Done {
            return;
        }
        if !handoff::task_has_verdict_gate(task) {
            return;
        }
        let verdict = handoff::read_inspect_verdict(task, work_dir, &self.state.project_root);
        let issues = handoff::collect_inspect_issues(task, work_dir, &self.state.project_root);
        let (blocked, blocking_n) =
            handoff::inspect_pass_blocked_by_issues(task, work_dir, &self.state.project_root);
        let _ = blocked; // folded into domain fail_reason via blocking_n
        let treat_unknown_as_fail =
            self.plan.require_inspect || task.role == Some(crate::plan::TaskRole::Inspect);

        let fail_reason = handoff::inspect_gate_fail_reason(
            verdict,
            blocking_n,
            issues.len(),
            treat_unknown_as_fail,
            &task.id,
        );

        let Some(reason) = fail_reason else {
            return;
        };
        result.status = TaskStatus::Failed;
        result.error = Some(reason.clone());
        warn!(
            task = %task.id,
            issues = issues.len(),
            blocking = blocking_n,
            ?verdict,
            "task failed: inspect gate ({reason})"
        );
    }

    /// P1-4: host merges fragment into global handoff (never written by worker).
    pub(super) fn handoff_task_end(
        &self,
        id: &str,
        result: &TaskResult,
        work_dir: Option<&std::path::Path>,
    ) {
        let Some(task) = self.plan.task(id) else {
            return;
        };
        if let Err(e) = handoff::on_task_end(&self.plan, &self.state, task, result, work_dir) {
            warn!(task = %id, err = %e, "handoff task_end failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::provider::TaskStatus;
    use crate::state::RunState;
    use std::path::PathBuf;
    use std::process::Command;

    fn git(repo: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git run");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn make_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "--initial-branch=main"]);
        git(dir.path(), &["config", "--local", "user.name", "test"]);
        git(dir.path(), &["config", "--local", "user.email", "t@t.t"]);
        std::fs::write(dir.path().join("README.md"), "# base\n").unwrap();
        git(dir.path(), &["add", "README.md"]);
        git(dir.path(), &["commit", "-m", "base"]);
        dir
    }

    fn implement_task(id: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: "codex".into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: Some(crate::domain::plan::TaskRole::Implement),
            scope: Some(crate::domain::plan::TaskScope {
                paths: vec!["src/**".into()],
                readonly: vec![],
                forbid: vec![],
            }),
            outputs: vec![],
            tags: vec![],
            wait_for: vec![],
        }
    }

    fn bare_sched(plan: crate::plan::PlanIR, state: RunState) -> Scheduler {
        Scheduler {
            max_parallel: 1,
            plan,
            state,
            registry: crate::runtime::provider::ProviderRegistry::from_providers(vec![
                std::sync::Arc::new(crate::runtime::provider::fake::FakeProvider::new(
                    "fake".into(),
                )),
            ])
            .expect("fake registry"),
            poll_interval: std::time::Duration::from_millis(5),
            yes: true,
            only: None,
            from_task: None,
            dry_run: false,
            mirror_state: None,
            auto_open_terminal: false,
            terminal_kind: crate::SessionKind::Embedded,
            terminal_manager: None,
            run_max_budget_usd: None,
            provider_max_parallel: Default::default(),
            retry_max: 0,
            stall_secs: 600,
            failover_enabled: false,
            fallback_extra_attempts: 1,
            failover_order: vec![],
            cost_escalate_enabled: false,
            browser: crate::config::BrowserConfig::default(),
            provider_unhealthy: Vec::new(),
            collab_bus: None,
            memory: None,
        }
    }

    #[test]
    fn success_evidence_fails_with_zero_diff_and_no_execution() {
        let repo = make_repo();
        let base_sha = git(repo.path(), &["rev-parse", "HEAD"]);
        let task = implement_task("t2");
        // Simulate a worktree-like directory whose HEAD == fork base and no changes.
        let wt = repo.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        git(repo.path(), &["worktree", "add", "-b", "wtbr", wt.to_str().unwrap(), "HEAD"]);

        let mut ir = crate::plan::PlanIR {
            schema: "cco-plan/v1".into(),
            name: "n".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("p.cco.yaml"),
            max_parallel: 1,
            on_failure: crate::plan::OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![task.clone()],
        };
        ir.tasks[0].depends_on = vec!["t1".into()];
        let state = RunState::new("r1".into(), repo.path().to_path_buf(), &ir, repo.path().join("run"));
        std::fs::create_dir_all(state.task_dir("t2")).unwrap();
        // Record fork_base_sha == current HEAD (worker did nothing).
        std::fs::write(
            state.task_dir("t2").join("work_dir.json"),
            format!("{{\"fork_base_sha\":\"{base_sha}\"}}"),
        )
        .unwrap();

        let mut result = TaskResult {
            status: TaskStatus::Done,
            exit_code: Some(0),
            stdout_path: None,
            session_id: None,
            agent_id: None,
            cost_usd: None,
            raw: serde_json::json!({}),
            error: None,
            done_marker: true,
            execution_evidence: false,
        };
        let s = bare_sched(ir, state);
        s.enforce_success_evidence(&task, &wt, &mut result);
        assert_eq!(result.status, TaskStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("空转"));
    }

    #[test]
    fn success_evidence_passes_inspect_role() {
        let repo = make_repo();
        let base_sha = git(repo.path(), &["rev-parse", "HEAD"]);
        let mut task = implement_task("t8");
        task.role = Some(crate::domain::plan::TaskRole::Inspect);
        let ir = crate::plan::PlanIR {
            schema: "cco-plan/v1".into(),
            name: "n".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("p.cco.yaml"),
            max_parallel: 1,
            on_failure: crate::plan::OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![task.clone()],
        };
        let state = RunState::new("r1".into(), repo.path().to_path_buf(), &ir, repo.path().join("run"));
        std::fs::create_dir_all(state.task_dir("t8")).unwrap();
        std::fs::write(
            state.task_dir("t8").join("work_dir.json"),
            format!("{{\"fork_base_sha\":\"{base_sha}\"}}"),
        )
        .unwrap();

        let mut result = TaskResult {
            status: TaskStatus::Done,
            exit_code: Some(0),
            stdout_path: None,
            session_id: None,
            agent_id: None,
            cost_usd: None,
            raw: serde_json::json!({}),
            error: None,
            done_marker: true,
            execution_evidence: true,
        };
        let s = bare_sched(ir, state);
        s.enforce_success_evidence(&task, repo.path(), &mut result);
        assert_eq!(result.status, TaskStatus::Done, "inspect role keeps Done");
    }

    #[test]
    fn success_evidence_passes_when_worker_committed() {
        let repo = make_repo();
        let base_sha = git(repo.path(), &["rev-parse", "HEAD"]);
        let task = implement_task("t2");
        // Worker commits a real change on its branch.
        let wt = repo.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        git(repo.path(), &["worktree", "add", "-b", "wtbr", wt.to_str().unwrap(), "HEAD"]);
        std::fs::write(wt.join("new.txt"), "real work\n").unwrap();
        git(&wt, &["add", "new.txt"]);
        git(&wt, &["commit", "-m", "t2 work"]);
        let _ = base_sha;

        let mut ir = crate::plan::PlanIR {
            schema: "cco-plan/v1".into(),
            name: "n".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("p.cco.yaml"),
            max_parallel: 1,
            on_failure: crate::plan::OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![task.clone()],
        };
        ir.tasks[0].depends_on = vec!["t1".into()];
        let state = RunState::new("r1".into(), repo.path().to_path_buf(), &ir, repo.path().join("run"));
        std::fs::create_dir_all(state.task_dir("t2")).unwrap();
        std::fs::write(
            state.task_dir("t2").join("work_dir.json"),
            format!("{{\"fork_base_sha\":\"{base_sha}\"}}"),
        )
        .unwrap();

        let mut result = TaskResult {
            status: TaskStatus::Done,
            exit_code: Some(0),
            stdout_path: None,
            session_id: None,
            agent_id: None,
            cost_usd: None,
            raw: serde_json::json!({}),
            error: None,
            done_marker: true,
            execution_evidence: true,
        };
        let s = bare_sched(ir, state);
        s.enforce_success_evidence(&task, &wt, &mut result);
        assert_eq!(result.status, TaskStatus::Done, "committed work stays Done");
    }

    #[test]
    fn success_evidence_fails_scope_none_empty_spin() {
        // 金样：scope=None 不再早退。可写 role、无执行证据、无 outputs、非 worktree → Failed。
        let repo = make_repo();
        let mut task = implement_task("t9");
        task.scope = None;
        let ir = crate::plan::PlanIR {
            schema: "cco-plan/v1".into(),
            name: "n".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("p.cco.yaml"),
            max_parallel: 1,
            on_failure: crate::plan::OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![task.clone()],
        };
        let state = RunState::new("r1".into(), repo.path().to_path_buf(), &ir, repo.path().join("run"));
        let mut result = TaskResult {
            status: TaskStatus::Done,
            exit_code: Some(0),
            stdout_path: None,
            session_id: None,
            agent_id: None,
            cost_usd: None,
            raw: serde_json::json!({}),
            error: None,
            done_marker: true,
            execution_evidence: false,
        };
        let s = bare_sched(ir, state);
        s.enforce_success_evidence(&task, repo.path(), &mut result);
        assert_eq!(result.status, TaskStatus::Failed, "scope=None + 空转应判 Failed");
        assert!(result.error.as_deref().unwrap_or("").contains("空转"));
    }

    #[test]
    fn success_evidence_passes_when_execution_evidence() {
        // 金样：有 command_execution（执行动作证据）→ Done，即使非 worktree / 无 outputs。
        let repo = make_repo();
        let mut task = implement_task("t10");
        task.scope = None;
        let ir = crate::plan::PlanIR {
            schema: "cco-plan/v1".into(),
            name: "n".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("p.cco.yaml"),
            max_parallel: 1,
            on_failure: crate::plan::OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![task.clone()],
        };
        let state = RunState::new("r1".into(), repo.path().to_path_buf(), &ir, repo.path().join("run"));
        let mut result = TaskResult {
            status: TaskStatus::Done,
            exit_code: Some(0),
            stdout_path: None,
            session_id: None,
            agent_id: None,
            cost_usd: None,
            raw: serde_json::json!({}),
            error: None,
            done_marker: true,
            execution_evidence: true,
        };
        let s = bare_sched(ir, state);
        s.enforce_success_evidence(&task, repo.path(), &mut result);
        assert_eq!(result.status, TaskStatus::Done, "有执行动作证据 → Done");
    }

}
