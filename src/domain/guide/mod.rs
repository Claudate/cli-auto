//! Guide domain: pure session / brief / pack types (G0-1 · contract shell).
//!
//! [INPUT]: presentation params (mode / entry / role pack id)
//! [OUTPUT]: GuideSession · GuideBrief · RolePack · SessionMode — serde types for store + DTO
//! [POS]: domain/guide — consumed by `app::guide` and `state::guide_store`; **no IO** here
//! [PROTOCOL]: 变更时更新此头部与 src/domain/CLAUDE.md · docs/guided-plan-memory-decision-2026-07-21.md
//!
//! G0 ships only the shape (types + golden JSON). Slot contracts, brief synthesis and
//! role-pack content arrive with G1/G2 — do not add strategy here.

use serde::{Deserialize, Serialize};

/// Session mode (`guide_sessions.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// 多角色对抗（角色卡并排提案 + 人勾选）
    Debate,
    /// 槽位追问（反问成 Brief · 自适应停）
    Coop,
    /// 用户主导
    UserLed,
}

impl SessionMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "debate" => Some(Self::Debate),
            "coop" => Some(Self::Coop),
            "user_led" => Some(Self::UserLed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debate => "debate",
            Self::Coop => "coop",
            Self::UserLed => "user_led",
        }
    }
}

/// Session entry (`guide_sessions.entry`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEntry {
    /// 快开始（少依赖画像 · 扩散 + 短对抗）
    Quick,
    /// 帮我想清楚（苏格拉底追问）
    Socratic,
    /// 导入已有材料
    Import,
    /// 已有计划（直接进入核对）
    ExistingPlan,
}

impl SessionEntry {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "quick" => Some(Self::Quick),
            "socratic" => Some(Self::Socratic),
            "import" => Some(Self::Import),
            "existing_plan" => Some(Self::ExistingPlan),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Socratic => "socratic",
            Self::Import => "import",
            Self::ExistingPlan => "existing_plan",
        }
    }
}

/// Session status (`guide_sessions.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// 进行中
    Active,
    /// 检查点暂停（等人闸）
    Checkpoint,
    /// 已出 Brief（阶段性总结完成）
    Synthesized,
    /// 放弃
    Abandoned,
}

impl SessionStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "checkpoint" => Some(Self::Checkpoint),
            "synthesized" => Some(Self::Synthesized),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Checkpoint => "checkpoint",
            Self::Synthesized => "synthesized",
            Self::Abandoned => "abandoned",
        }
    }
}

/// One role inside a pack (G2-1 ships real packs; G0 only the shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleSpec {
    pub id: String,
    pub label: String,
    /// 需求层标签（safety|growth|social|actualization|reality|…）—— 可选
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub need_tag: Option<String>,
}

/// Role pack: id + label + ordered roles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolePack {
    pub id: String,
    pub label: String,
    pub roles: Vec<RoleSpec>,
}

/// 阶段性 Brief（问题重述 · 诉求地图 · 选项 · 得/失 · 风险 · 倾向 · 未决 · 验收 · V1）。
/// Stored as `guide_sessions.brief_json`; presented to user for editing before plan.md.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuideBrief {
    /// 问题重述
    pub restatement: String,
    /// 给谁
    pub audience: String,
    /// 选项
    #[serde(default)]
    pub options: Vec<String>,
    /// 得/失（tradeoffs）
    #[serde(default)]
    pub tradeoffs: Vec<String>,
    /// 风险
    #[serde(default)]
    pub risks: Vec<String>,
    /// 倾向性建议
    #[serde(default)]
    pub recommendation: String,
    /// 未决问题
    #[serde(default)]
    pub open_questions: Vec<String>,
    /// 怎样算做完（验收）
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// V1 边界
    #[serde(default)]
    pub v1_scope: Vec<String>,
}

/// One guided session row (G0 shell: list / start / get).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuideSession {
    pub session_id: String,
    pub project: String,
    pub mode: SessionMode,
    pub entry: SessionEntry,
    pub status: SessionStatus,
    /// Role pack id (content arrives G2-1).
    pub role_pack: String,
    /// 槽位 JSON（G1-2 defines the slot contract; free-form until then）
    #[serde(default = "empty_object")]
    pub slots: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<GuideBrief>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden JSON: a complete session with brief (shape contract; keep in sync with docs §6).
    const GOLDEN_SESSION_JSON: &str = r#"{
      "session_id": "20260804T120000Z-g001",
      "project": "/tmp/demo",
      "mode": "coop",
      "entry": "socratic",
      "status": "synthesized",
      "role_pack": "ship-product",
      "slots": { "goal": "提醒浇水的工具", "audience": "阳台种植新手" },
      "brief": {
        "restatement": "做一个轻量浇水提醒工具",
        "audience": "阳台种植新手",
        "options": ["桌面提醒", "微信机器人", "仅手动清单"],
        "tradeoffs": ["桌面快但不可移动 / 微信可达但需外发"],
        "risks": ["提醒疲劳", "过度依赖传感器"],
        "recommendation": "先做桌面提醒 + 手动清单",
        "open_questions": ["是否接天气数据"],
        "acceptance": ["能设浇水频率并按时提醒", "漏提醒有补记"],
        "v1_scope": ["单植物档案", "桌面提醒"]
      },
      "created_at": "2026-08-04T12:00:00+08:00",
      "updated_at": "2026-08-04T12:30:00+08:00"
    }"#;

    #[test]
    fn golden_session_json_parses_and_roundtrips() {
        let s: GuideSession = serde_json::from_str(GOLDEN_SESSION_JSON).expect("golden parses");
        assert_eq!(s.mode, SessionMode::Coop);
        assert_eq!(s.entry, SessionEntry::Socratic);
        assert_eq!(s.status, SessionStatus::Synthesized);
        assert_eq!(s.role_pack, "ship-product");
        let brief = s.brief.as_ref().expect("brief present");
        assert_eq!(brief.options.len(), 3);
        assert_eq!(brief.acceptance.len(), 2);
        assert!(s.slots.get("goal").is_some());

        let back = serde_json::to_string(&s).expect("serializes");
        let again: GuideSession = serde_json::from_str(&back).expect("round-trip parses");
        assert_eq!(again, s);
    }

    #[test]
    fn mode_entry_status_parse_and_as_str() {
        for (raw, mode) in [
            ("debate", SessionMode::Debate),
            ("coop", SessionMode::Coop),
            ("user_led", SessionMode::UserLed),
        ] {
            assert_eq!(SessionMode::parse(raw), Some(mode));
            assert_eq!(mode.as_str(), raw);
        }
        assert_eq!(SessionMode::parse("nope"), None);
        for (raw, entry) in [
            ("quick", SessionEntry::Quick),
            ("socratic", SessionEntry::Socratic),
            ("import", SessionEntry::Import),
            ("existing_plan", SessionEntry::ExistingPlan),
        ] {
            assert_eq!(SessionEntry::parse(raw), Some(entry));
            assert_eq!(entry.as_str(), raw);
        }
        assert_eq!(
            SessionStatus::parse("checkpoint"),
            Some(SessionStatus::Checkpoint)
        );
        assert_eq!(SessionStatus::parse("x"), None);
    }

    #[test]
    fn role_pack_golden_shape() {
        let pack: RolePack = serde_json::from_str(
            r#"{"id":"ship-product","label":"产品出海","roles":[
                {"id":"market","label":"市场","need_tag":null},
                {"id":"delivery","label":"交付"}
            ]}"#,
        )
        .expect("pack parses");
        assert_eq!(pack.roles.len(), 2);
        assert_eq!(pack.roles[1].need_tag, None);
    }
}
