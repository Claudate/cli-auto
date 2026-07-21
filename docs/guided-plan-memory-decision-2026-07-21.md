# 引导成长计划 · 用户记忆 · 多视角对抗 · 与 cco 结合方案

> 日期：2026-07-21  
> 角色：**工程形状草稿**（引导写计划 + SQLite 会话记忆 + 有界对抗）——**产品本体以** [`subjective-desire-decision-concept.md`](./subjective-desire-decision-concept.md) **为准**  
> 状态：**设计稿 · 未排期实施**（勾选 ☐ 见 §8；**不**平行第二套 A0–A5；心爱主线是「接近主观渴望」，本文偏 cco 嫁接想象）  
> 产品方向：[`../PRODUCT.md`](../PRODUCT.md)  
> 架构边界：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)（confirm 唯一开跑 · MVVM · 体积）  
> 拆分 SoT：[`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md)  
> 拆分 Agent：[`split-agent-model-path-2026-07-21.md`](./split-agent-model-path-2026-07-21.md) · [`openhands-style-split-agent-landing-2026-07-21.md`](./openhands-style-split-agent-landing-2026-07-21.md)  
> 体验：[`ux-nondev-landing-2026-07-21.md`](./ux-nondev-landing-2026-07-21.md)  
> 聊天共建（已归档参考）：[`archive/chat-plan-builder-2026-07-18.md`](./archive/chat-plan-builder-2026-07-18.md)

[PROTOCOL]: 改本方案时同步 §0 定位、§6 数据模型、§8 波次勾选；落地后回写 PRODUCT 五步叙事（只在「① 生成」前插入引导子相）与 `docs/CLAUDE.md` 索引。**禁止**旁路 `confirm_start`；**禁止**把本文件写成第二套调度器。

---

## 0. 一句话与产品定位

### 0.1 要解决什么

今天的 cco 主路径是：

```text
用户丢一句想法 / 聊几轮 → 直接落 plan.md → Mode B 拆分 → 确认 → 跑
```

问题不在「拆/跑」，而在 **① 生成** 之前：

| 现状 | 后果 |
|------|------|
| 思路 → 计划 一步跳 | 用户没想清目标/约束/取舍，计划空心、拆分再漂亮也白跑 |
| 会话无长期记忆 | 每次冷启动；AI 重复问；无法「预判你可能关心什么」 |
| 无多视角对抗 | 计划像单人草稿，盲点与风险不暴露 |
| 默认像「答案机」 | 与主受众（PM/出海）真实工作方式不符：他们需要**可核对的权衡**，不是一口论断 |

### 0.2 我们是什么（对齐你的四段原话）

你不是要一个 **答案许愿机**，而是：

> **决策可视化控制台**：用提问把思路捋顺 → 暴露盲点与「会失去什么」→ 给出**倾向性建议**与可执行计划 → 人确认后才拆开跑。

这与 PRODUCT「任务控制台」不冲突，而是把五步主循环的 **① 生成计划** 做成**引导相**，而不是替换 ③–⑤ 的编排内核。

```text
【新增】引导相 Guide
  冷启动入口 → 苏格拉底追问 / 快启动 / 可选画像
  → 多角色领取诉求（需求层 / 角色组）
  → 有界多轮发言 + 检查点（人闸 or 主持人评分）
  → 阶段性总结 = 结构化 Brief
      ↓
【既有】① plan.md 落盘
【既有】② 核对
【既有】③ Split（CcoSplit + SQLite SoT）→ confirm 唯一开跑
【既有】④ Run / ⑤ Inspect
```

### 0.3 非目标

| 不做 | 原因 |
|------|------|
| 全量「心理咨询 / 日记向量人格 OS」首版 | 体量大、合规与效果未验证；作 **入口 c 可选**，不挡主路径 |
| 自由无限群聊 + 无界对抗 | 易过度设计；首版 **固定轮次 + 角色卡 + 检查点** |
| 静默 auto 跳过引导或 optional | 与 optional/confirm 硬契约一致 |
| 旁路 Mode B / 第二套 Scheduler | L1 硬规则 |
| 主路径塞引擎名、评分 JSON、run_id | PRODUCT 主受众文案 |
| 云端多租户用户画像 SaaS | cco = 本机；记忆落 `~/.cco/cco.db` |

---

## 1. 你的四段意图 → 产品能力映射

| # | 你的原意（压缩） | 产品能力名 | 落点 |
|---|------------------|------------|------|
| 1 | N 轮角色发言；轮末主持人审「是否创造价值」；低分干预；终轮总结最高分侧 | **有界对抗 · 轮次检查点** | Guide 相；默认 **人闸按钮** MVP，主持人自动评分作增强 |
| 1b | 对抗 / 合作 / 妥协 / 用户介入 / 互助会 / 倾诉 多规则 | **会话模式（Session Mode）** | 枚举配置；首版只落地 **对抗 + 合作 + 用户介入** |
| 2 | 0 数据冷启动：快启动 / 测题 / 反问 / 生活记录向量 | **冷启动三入口** | 快启动 + 反问 = P0；测题 = P1；向量日记 = P2 以后 |
| 3 | 不是许愿机；追问理清思路；暴露盲点与失去什么；倾向建议、接受不完美 | **决策可视化原则** | Brief 输出结构强制含：选项 / 得 / 失 / 风险 / 未决 |
| 4 | 「核心利益」= 需求层角色（安全/成长/社交/自我实现/现实…）；另有感性组、理性组 | **角色组（Role Pack）** | 模板库；按问题类型推荐一组，可换组不重训模型 |

---

## 2. 外部对标（网络检索综合 · 2025–2026）

> 下列为**模式借鉴**，不换栈、不抄产品壳。链接便于复核。

### 2.1 多代理辩论与决策支持

| 来源 | 可借 | 不借 |
|------|------|------|
| [Multi-Agent Debate (MAD) 研究线](https://arxiv.org/) · 产业表述 | 多角色对立 → 降幻觉、提稳健；**有界轮次 + 裁判/合成** | 无界自由辩论 |
| [Microsoft AutoGen GroupChat](https://microsoft.github.io/autogen/0.2/docs/Use-Cases/agent_chat/) | `human_input_mode` ALWAYS/TERMINATE/NEVER；**FSM 约束发言顺序**；GroupChatManager 选下一说话人 | 整包 AutoGen 运行时 |
| [CrewAI hierarchical](https://docs.crewai.com/) · [LangGraph plan-and-execute](https://www.langchain.com/blog/planning-agents) | Manager 拆任务 / Planner 与 Executor 分节点 | 引入 Python 图运行时 |
| [Genspark Super Agent / Multi-Agent](https://www.genspark.ai/agents) | 多专长协作出交付物；持久上下文（AI Drive 类） | 云端「超级代理」叙事抢 cco 轻量定位 |
| [OpenHands Plan Mode](https://www.openhands.dev/)（本仓已对照） | **Plan 与 Execute 分离 + 人确认** | 整站 Agent IDE |

### 2.2 苏格拉底追问 · 冷启动画像

| 模式 | 启示 |
|------|------|
| Socratic product discovery / clarifying questions | **先问再写计划**；问题自适应（上一答决定下一问） |
| LLM cold-start profiling | 短问答 + 结构化槽位（目标/约束/资源/成功标准）比长问卷更适合主路径 |
| 测题 / 日记向量 | 高信息密度但高摩擦 → **可选支路**，不得挡「快启动」 |

### 2.3 Advisor / Coach / Delegate 分权

来自 [Choose Your Agent: Advisor / Coach / Delegate 交易权衡研究](https://arxiv.org/html/2602.12089v3)（多人谈判实验）：

| 模态 | 含义 | 对 cco 建议 |
|------|------|-------------|
| **Advisor** | AI 先提建议，人接受/改/拒 | **默认**：角色卡片并排提案 + 人勾选 |
| **Coach** | 人先写，AI 批评 | 用户改 Brief / 计划后的「挑刺」一轮 |
| **Delegate** | AI 直接提交 | **仅**低风险步 + **短否决窗**；绝不当主路径默认 |

**关键发现**：用户偏好控权（Advisor），但人的过滤会稀释 AI 质量；设计要让高质量提案**看得见、一键采用**，而不是逼人重写。

### 2.4 与 cco 差异化（我们不做成谁）

| 产品型 | 他们 | cco 坚持 |
|--------|------|----------|
| 聊天机器人 | 对话即答案 | 对话 → **Brief + plan.md** → 拆跑 |
| Agent IDE | 写代码/PR 为中心 | 计划文档一等公民 · 非开发文案 |
| 人生 OS / 倾诉 App | 人格向量长期陪伴 | 本机项目记忆 + 可选画像；**不**默认心理产品 |
| 纯 MAD 研究原型 | 群聊表演 | **检查点可视 + 落计划可执行** |

---

## 3. 现状盘点（地形）

### 3.1 已有能力（可复用）

| 层 | 现状 | 对本方案 |
|----|------|----------|
| 聊天共建 | `app::chat`：session JSON · send · save_plan · fence 抽 plan | **Guide 可挂在 chat 前相或并列相**；最终仍 `save_plan` → `.md` |
| Mode B | plan job · LLM/heuristic · confirm 唯一开跑 | **不改**；Brief 变厚 plan.md 即可 |
| CcoSplit + SQLite | `cco.db`：`plan_jobs` / `plan_tasks` / `cco_split_*` | **扩展表**做用户/会话记忆；拆分 SoT 继续独立 |
| 拆分 Agent 路径 | ModelSplitAgent 设计稿 | Guide 产出的结构字段可写入 plan front-matter，供拆分提示词使用 |
| optional / 停台 | 业务可选勾选 · 默认停拆分台 | Guide 结束「生成计划」后仍走核对/拆分闸 |

### 3.2 缺口

| 缺口 | 说明 |
|------|------|
| **无引导相** | 聊天偏「共建散文」，不是结构化追问槽位 |
| **无用户记忆表** | SQLite 只有 job/task；会话在 `.cco/chat/*.json` 文件，无跨会话预判 |
| **无角色组 / 轮次状态机** | 多 CLI 的 role/scope 是 **Worker 路由**，不是「需求层人格角色」 |
| **无决策 Brief  schema** | 计划 md 无强制「得/失/风险/未决」 |
| **无轮次评分持久化** | 无法复盘「哪一轮创造了价值」 |

### 3.3 架构硬约束（实施时不可破）

1. Presentation → App → Domain；Domain 不依赖 UI/Tauri。  
2. 唯一业务开跑：`split::confirm` / `confirm_start`。  
3. 策略不进 JS View；IPC 只 gateway。  
4. 文件软 400 / 硬 600；禁止堆 `state.js` / classic facade。  
5. 同一屏新概念 ≤ 3（引导相 UI 要拆屏或折叠高级模式）。

---

## 4. 目标用户流程（人话）

### 4.1 主路径：引导 → 计划 → 拆跑（默认）

```text
打开项目
  → 入口三选一（概念 ≤3）：
       [快开始]  [帮我想清楚]  [用已有计划]
  → 若「帮我想清楚」：
       1–2 句描述问题
       → 系统推荐角色组（可换）
       → 反问 3～7 轮（自适应停）
       → （可选）多角色短对抗 2～5 轮 · 每轮可「有价值 / 偏了 / 我插一句」
       → 出「阶段性 Brief」：问题重述 · 诉求地图 · 选项 · 得/失 · 风险 · 倾向 · 未决
       → 用户改两句 →「写成计划」→ plan.md
  → 若「快开始」：
       问题扩散拆解（少依赖画像）→ 问题解决型角色组短对抗 → 直接成 plan.md
  → 既有：核对 → 拆成步骤 → 确认并开始 → 进度 → 巡检
```

### 4.2 会话预判（记忆）

```text
再次打开同项目 / 新会话：
  顶栏或空态一行人话（非 debug）：
  「上次你卡在：成本 vs 上线时间；角色组偏增长。要沿用还是重来？」
  数据来自 SQLite user_memory + 最近 guide_session 摘要
```

### 4.3 与「辞职」类决策题的兼容

cco 主受众是 **项目任务**（落地页、活动、PRD），但同一套 **Guide 状态机**可服务「个人决策」：

| 问题类型 | 角色组模板 | 输出是否进 Run |
|----------|------------|----------------|
| 产品/出海任务 | 理性组：市场/交付/风险/文案… | **是** → Split → Worker |
| 个人决策（辞职等） | 需求层：安全/成长/社交/自我/现实… | **默认可停在 Brief**；「写成行动计划」才进 Run |

产品上：**同一引导引擎，两种落点**（Brief-only vs Brief→Plan→Run）。主路径 CTA 仍偏「任务可跑」。

---

## 5. 核心设计

### 5.1 两层「角色」不要混

| 层 | 名字 | 含义 | 已有/新建 |
|----|------|------|-----------|
| **A. 诉求角色（Guide）** | Role Pack 里的「安全需求」「市场分析师」… | 辩论与领取诉求 | **新建** |
| **B. Worker 路由角色** | `provider` / `role` / `scope` | 谁去执行任务 | **已有** multi-cli |

Guide 结束物化 plan 时：诉求角色 → 计划章节与验收标准；**不要**把「安全需求」直接写成 Worker role 字符串（除非用户显式映射高级选项）。

### 5.2 会话模式（Session Mode）

首版枚举（可配置，验证后再加）：

| mode | 行为 | 人闸 |
|------|------|------|
| `debate` | 对立领取诉求 · 互相反驳 | 每轮「有价值/偏了」 |
| `coop` | 补全信息 · 少反驳 | 每轮「够了/再问」 |
| `user_led` | 用户先发言，角色只追问 | 用户点「下一角色」 |
| （以后）`compromise` / `support_circle` / `listen` | 规则包 | 同检查点框架 |

**MVP 默认**：任务类 → `coop` 追问为主 + 短 `debate` 风险轮；决策类 → `debate` 为主。

### 5.3 轮次检查点（你提的 5 轮方案 · 工程化）

你的设想：

```text
每轮角色依次发言 → 主持人审价值 → 低分干预 → 终轮总结高分侧
```

过度设计风险真实存在。采用 **双轨**：

| 轨 | 行为 | 何时用 |
|----|------|--------|
| **轨 H · 人闸（默认 MVP）** | 每轮结束 2～3 按钮：`有价值 · 偏了 · 我补充`；「偏了」触发收窄提示词 | 未验证评分前 **唯一默认** |
| **轨 A · 主持人 AI（增强）** | 轮末结构化打分：`novelty / on_topic / actionable / user_aligned`；低于阈值 → 插入干预发言或缩短后续轮 | 实验开关；**不得**无声自动否决用户 |

固定参数（可配置，默认保守）：

```text
max_rounds = 3～5（任务 3 / 深决策 5）
speakers_per_round = min(角色数, 4)   # 防止角色爆炸
checkpoint = 每轮必停（H）或 评分+摘要（A）
synthesis = 最后一轮后强制 Brief，禁止再开无限轮
```

**终局总结**：不是只摘「评分最高 AI 全文」，而是：

1. 各角色 **终态立场卡**（3 句内）  
2. **冲突地图**（哪两方对立）  
3. **倾向建议**（标明不确定）  
4. **行动项候选**（可勾选进 plan.md）

评分最高侧作 **加权**，不是唯一真相源——对齐「无唯一最优解」。

### 5.4 冷启动三入口（你的 2a/2b/2c）

```text
                    ┌─ [快开始] ── 问题扩散 ── 解决型角色短对抗 ── Brief/Plan
用户打开空项目 ────┼─ [帮我想清楚] ── 反问槽位 ──（可选角色对抗）── Brief ── Plan
                    └─ [用已有计划] ── 跳过 Guide（现状）
可选折叠：
  · 短测题（2a）→ 写入 profile 槽位
  · 导入日记/录音（2c）→ 本地摘要（非必向量）；P2
```

**反问槽位（2b）最小集合**（自适应可跳过已填）：

| 槽位 | 人话问法示例 |
|------|----------------|
| goal | 你真正想推进/决定的是什么？ |
| success | 怎样算「这周没白做」？ |
| constraint | 预算/时间/人手/合规硬限制？ |
| loss | 若做成/若做成另一选择，你最怕失去什么？ |
| stakeholders | 还要让谁满意或会受影响？ |
| non_goal | 明确不做的是什么？ |

### 5.5 Brief 输出契约（决策可视化核心）

`GuideBrief`（内存 + SQLite JSON + 可渲染 md）：

```text
# 议题重述（1 段）
# 诉求地图（角色 → 诉求 · 优先级）
# 选项（≥2；允许「暂缓」）
# 每个选项：得到 / 失去 / 风险 / 前置条件
# 倾向建议（非论断；置信说明）
# 未决问题（必须可见）
# 可执行下一步（勾选 → 写入 plan.md 任务雏形）
```

写入 `plan.md` 时用固定章节标题，便于 Mode B / Split Agent 提示词抓取。

### 5.6 用户记忆（SQLite）

路径：现有 `~/.cco/cco.db`（与 cco_split 同库，分表）。

```sql
-- 本机用户级（跨项目弱画像）
CREATE TABLE IF NOT EXISTS user_profile (
  profile_id TEXT PRIMARY KEY,          -- 默认 'local'
  display_name TEXT,
  prefs_json TEXT NOT NULL DEFAULT '{}', -- 语言/默认模式/是否启用主持人评分
  traits_json TEXT NOT NULL DEFAULT '{}',-- 渐进槽位：风险偏好、常关注维度…
  updated_at TEXT NOT NULL
);

-- 项目级记忆（强相关）
CREATE TABLE IF NOT EXISTS project_memory (
  project TEXT PRIMARY KEY,
  summary TEXT NOT NULL DEFAULT '',     -- 人话摘要，供预判条
  open_tensions_json TEXT NOT NULL DEFAULT '[]', -- 未解冲突
  last_role_pack TEXT,
  last_brief_json TEXT,
  signals_json TEXT NOT NULL DEFAULT '{}', -- 计数：常改预算、常否可选…
  updated_at TEXT NOT NULL
);

-- 一次引导会话
CREATE TABLE IF NOT EXISTS guide_sessions (
  session_id TEXT PRIMARY KEY,
  project TEXT NOT NULL,
  mode TEXT NOT NULL,                  -- debate|coop|user_led
  entry TEXT NOT NULL,                 -- quick|socratic|import|existing_plan
  status TEXT NOT NULL,                -- active|checkpoint|synthesized|abandoned
  role_pack TEXT NOT NULL,
  slots_json TEXT NOT NULL DEFAULT '{}',
  brief_json TEXT,
  plan_path TEXT,                      -- 物化后的 plan.md
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_guide_sessions_project
  ON guide_sessions(project, updated_at DESC);

-- 轮次与发言（可审计 · 可复盘）
CREATE TABLE IF NOT EXISTS guide_rounds (
  session_id TEXT NOT NULL,
  round_idx INTEGER NOT NULL,
  checkpoint_kind TEXT NOT NULL,       -- human|host_ai
  human_verdict TEXT,                  -- valuable|off|user_note
  host_scores_json TEXT,               -- 轨 A
  intervention TEXT,                   -- 低分干预文案
  summary TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (session_id, round_idx)
);

CREATE TABLE IF NOT EXISTS guide_utterances (
  session_id TEXT NOT NULL,
  round_idx INTEGER NOT NULL,
  seq INTEGER NOT NULL,
  speaker_id TEXT NOT NULL,            -- role id or 'user' or 'host'
  role_label TEXT,
  need_tag TEXT,                       -- safety|growth|social|actualization|reality|…
  content TEXT NOT NULL,
  meta_json TEXT NOT NULL DEFAULT '{}',
  PRIMARY KEY (session_id, round_idx, seq)
);
```

**预判条算法（规则优先，LLM 可选润色）**：

```text
if project_memory.summary non-empty:
  show: summary + open_tensions top1 + last_role_pack
else if user_profile.traits has domain:
  show: 弱预判（「你常关注交付风险」类）
else:
  show: 冷启动三入口，无假预判
```

隐私：本机库；设置页「清除项目记忆 / 清除本机画像」；**不**默认上传。

### 5.7 应用层落点（六边形）

```text
web features/guide/          View：入口 · 追问 · 角色卡 · 检查点按钮 · Brief
     │ gateway
     ▼
app::guide                   用例：start / answer_slot / run_round /
                             checkpoint / synthesize / materialize_plan
     │
     ├─ domain::guide        纯：RolePack · SessionMode · Brief · 评分规则
     ├─ ports::GuideLlmPort  生成问题/角色发言/主持人分/综合（Messages HTTP 优先）
     └─ state::guide_store   SQLite 读写（扩 sqlite.rs 或 guide_store.rs）

materialize_plan → 既有 app::chat::save_plan 或直接写 plans/guide-*.md
                 → 用户点「拆成步骤」→ 既有 split::start_job
```

**不要**把 Guide 状态机塞进 `plan/planner` 或 Scheduler。

### 5.8 角色组模板（首批）

**Pack `needs-maslow-lite`（个人/综合决策）**

| id | 标签 | 领取 |
|----|------|------|
| safety | 安全 | 现有生活/收入/稳定 |
| growth | 成长 | 发展空间 |
| social | 关系 | 朋友/爱人/团队 |
| actualize | 自我实现 | 梦想/意义 |
| reality | 现实事务 | 期限/债务/签证/健康硬约束 |

**Pack `ship-product`（PM/出海任务 · 主受众默认）**

| id | 标签 | 领取 |
|----|------|------|
| outcome | 结果 | 指标与成功标准 |
| user | 用户 | 受众与路径 |
| delivery | 交付 | 范围/工期/依赖 |
| risk | 风险 | 合规/品牌/技术债 |
| resource | 资源 | 人/钱/渠道 |

**Pack `feel-mind`（可选）**：欲望 / 恐惧 / 依恋 / 自我 —— 默认折叠。

问题分类器（小模型或规则）：关键词 + 用户选「这是工作任务 / 人生选择」→ 推荐 Pack，可一键换。

---

## 6. 与现有五步 / 拆分 / 记忆的关系图

```text
┌────────────── Guide（新）─────────────────┐
│ entry · slots · role pack · rounds · Brief │
│ SQLite: guide_* + project_memory           │
└──────────────────┬────────────────────────┘
                   │ materialize
                   ▼
            plans/*.md（① 生成）
                   │
                   ▼
            ② 核对（人改 md）
                   │
                   ▼
         ③ CcoSplit SoT（已有 SQLite）
                   │ confirm_start
                   ▼
            ④ Run Workers · ⑤ Inspect
```

**数据 SoT 分工**：

| 数据 | SoT |
|------|-----|
| 引导过程 / 轮次 / 发言 | `guide_*` SQLite |
| 项目预判摘要 | `project_memory` |
| 计划正文 | `plans/*.md` 文件 |
| 拆分任务图 | `cco_split_*` SQLite |
| 执行状态 | run 目录 / 既有 state |

---

## 7. 风险与验证（你已点出的诚实点）

| 风险 | 缓解 |
|------|------|
| 过度设计：5 轮+主持人+多规则未验证 | **轨 H 默认**；轨 A 开关；模式先 3 个 |
| 主持人评分「太短测不了」 | 先离线金样对话集 + 人工标签，再上线 |
| 引导过长劝退非开发 | 快开始 ≤3 分钟；帮我想清楚默认 ≤7 问；进度「还差 2 问」 |
| 像心理咨询产品偏移 | 主 CTA 任务语言；个人 Pack 不默认 |
| CLI 规划仍卡 | Guide **不**走 Claude CLI 长规划；用 Messages/HTTP 短调用（P2-7 路径） |
| 记忆污染 | 预判条可「忽略本次」；衰减/手动清 |
| 概念爆炸 | 首屏只暴露 3 入口；角色细节进第二屏 |

**最小验证协议（工具没做完前的纸面/原型测）**：

1. 5 个真实 PM/出海任务 + 2 个个人决策；只跑 Guide→Brief，不跑 Worker。  
2. 指标：用户是否说「比直接生成计划更清楚」；Brief「失去什么」是否被点名有用；完成时长。  
3. 人闸 vs 自动主持人 A/B 各 5 场，再决定默认轨。

---

## 8. 落地波次（建议 · 勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成  
> **不**重开 A0–A5；可与 `ux-nondev-landing` 波次 B（写计划顺滑）衔接。

### 波次 G0 · 契约与空壳（0.5–1 d）

| ID | 内容 | 状态 |
|----|------|------|
| G0-1 | Domain：`GuideBrief` · `RolePack` · `SessionMode` 类型 + 金样 JSON | ☐ |
| G0-2 | SQLite schema：`user_profile` / `project_memory` / `guide_*` + store 读写 | ☐ |
| G0-3 | `app::guide` 空用例 + Tauri/gateway 桩（list/start/get） | ☐ |
| G0-4 | 本文 + docs L2 索引；PRODUCT「① 生成」加一句引导子相（不改五步序） | ☐ |

### 波次 G1 · 反问成 Brief（主价值 · 2–3 d）

| ID | 内容 | 状态 |
|----|------|------|
| G1-1 | 冷启动入口 UI：快开始 / 帮我想清楚 / 已有计划 | ☐ |
| G1-2 | 槽位追问（coop）+ 自适应停 + slots 持久化 | ☐ |
| G1-3 | `synthesize` → Brief 人话页（得/失/风险/未决） | ☐ |
| G1-4 | `materialize_plan` → `plans/guide-*.md` + 跳转核对 | ☐ |
| G1-5 | 项目预判条（读 `project_memory`） | ☐ |

### 波次 G2 · 角色卡 + 人闸轮次（2–3 d）

| ID | 内容 | 状态 |
|----|------|------|
| G2-1 | Pack：`ship-product` + `needs-maslow-lite` | ☐ |
| G2-2 | 每轮 ≤4 发言 · 检查点按钮 · 落 `guide_rounds/utterances` | ☐ |
| G2-3 | 冲突地图可视化（简表即可，非大图表演） | ☐ |
| G2-4 | 快开始：扩散 + 短 debate 一轮 | ☐ |

### 波次 G3 · 主持人评分实验（可选 · 1–2 d）

| ID | 内容 | 状态 |
|----|------|------|
| G3-1 | 轨 A 开关 · 四维分 · 低分干预文案 | ☐ |
| G3-2 | 离线金样 + 简单校准报告 | ☐ |

### 波次 G4 · 增强（以后）

| ID | 内容 | 状态 |
|----|------|------|
| G4-1 | 短测题写入 traits | ☐ |
| G4-2 | 日记/录音 → 本地摘要（慎向量） | ☐ |
| G4-3 | 更多 Session Mode 规则包 | ☐ |
| G4-4 | Brief 与 Split Agent 提示词字段打通 | ☐ |

---

## 9. 实施时文件建议（防堆厚）

| 路径 | 职责 |
|------|------|
| `src/domain/guide/` | 纯类型 · 规则 · Brief 校验 |
| `src/app/guide.rs` | 用例 |
| `src/ports/guide_llm.rs` | Port |
| `src/state/guide_store.rs` | SQLite |
| `src/runtime/provider/guide_llm.rs` 或复用 Messages | Adapter |
| `web/js/features/guide/` | 入口 · 追问 · 角色卡 · Brief |
| `web/js/shared/gateway.js` | 仅增 guide* API |
| `docs/guided-plan-memory-decision-2026-07-21.md` | 本真源 |

禁止：往 `planner/llm.rs`、`services/chat` 上帝文件、`state.js` 堆策略。

---

## 10. 成功标准（产品语言）

1. **冷启动**：无历史时，3 入口清晰；无假预判。  
2. **引导**：用户不写长 PRD 也能产出含「得到/失去/风险/未决」的 Brief。  
3. **记忆**：二次进入同项目，一句话预判可关可沿用。  
4. **对抗**：默认 ≤5 轮、每轮可停；不出现无界群聊。  
5. **闭环**：Brief → plan.md → 拆分台 → confirm 开跑 **零旁路**。  
6. **轻量**：主路径不出现评分 JSON / provider 名；高级可展开「查看本轮发言」。  
7. **定位**：用户能说出「这是帮我想清楚再干活，不是替我做决定」。

---

## 11. 决议摘要（给实施/评审）

1. **引导相插在 ① 之前**，不改五步顺序与 confirm 契约。  
2. **默认人闸检查点**；主持人 AI 评分后置验证。  
3. **SQLite 扩展用户/项目/引导会话记忆**，与 `cco_split_*` 同库分表。  
4. **角色组 = 诉求领取**，与 Worker role 分离。  
5. **输出强制决策可视化 Brief**，再物化计划。  
6. **快开始不依赖画像**；深引导渐进丰富 `project_memory`。  
7. **先 G0–G1 ship**，再 G2 角色对抗；G3/G4 不挡主价值。

---

## 12. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-21 | 初稿：结合用户四段意图 · cco 现状 · 外部 MAD/Socratic/AutoGen/Genspark/Advisor 研究 · SQLite 记忆 · 波次 G0–G4 |
