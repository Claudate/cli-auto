# 本波执行 · 多窗口并发提示词（可直接粘贴）

> 勾选真源：[docs/path-depth-wave-2026-07-28/landing.md](../../docs/path-depth-wave-2026-07-28/landing.md)  
> 决策：[docs/path-depth-wave-2026-07-28/INDEX.md](../../docs/path-depth-wave-2026-07-28/INDEX.md)  
> 日期：2026-07-28  
> 状态：按 landing **W0 全 ✅ · W1 余 W1-6 · W2–W4 ☐** 拆窗

---

## 0. 调度总览（先看再开窗）

### 0.1 已完成（各窗禁止重做当主任务）

W0-1…W0-8（空态、去三档英雄键、场景芯片、草稿 CTA）  
W1-0…W1-5（当前理解、这版作数、反馈改、prompt）  
文档 01–06 / 01b 决策稿  

### 0.2 词汇（全窗统一 · 禁止教用户三档）

| 对内 | 对外 |
|------|------|
| `delivery`: trial / single / bundle | 不出现；只表现行为 |
| 画像 / 场景芯片 | 人话例子可点 |
| P1–P6 | 禁止 UI/用户文案 |
| ~~L/M/H~~ | 废名；禁止新文案 |

### 0.3 红线（全窗）

1. 开跑只经 `confirm_start` / Split.confirm；聊天/认领/保存/拆步 ≠ 开跑  
2. 不旁路 Mode B；不新建上帝 Manager  
3. 主路径无人话第一句引擎名 / run_id / VERDICT / P 代号 / L/M/H  
4. 文件软 400 / 硬 600；厚文件只抽不堆  
5. JS 业务策略不进 View；IPC 只 gateway  
6. **scope 不重叠**的窗才并行；下表 scope 互斥  

### 0.4 并发波次

```text
波次 α（立刻 4 窗并行）
  A  W1-6 桌面抽检 + 仅修抽检发现的 chat 文案/小 bug
  B  W2-4 supersede 按 plan_path（Rust planner）
  C  W2 域模型薄层：WorkBundle / Index 约定 + app 骨架（不碰 UI 大改）
  D  文档关账薄：PRODUCT 受众链 01 + landing 指针（纯 docs）

波次 β（等 B+C 主路径可测后 · 3 窗）
  E  W2-1…W2-3 需求包→索引→N 计划（接 C 的 API）
  F  W2-5 分计划拆分隔离 + 单计划重拆
  G  W3-1…W3-2 总览 DTO + Bundle 确认语义（可先假数据）

波次 γ（等 E/F 后 · 2 窗）
  H  W3-3…W3-4 执行串行默认 + per-plan 巡检入口
  I  W4 金样/arch/状态行关账
```

---

## 波次 α · 四窗并行（现在开）

---

### 窗口 A · W1-6 桌面抽检与热修

**角色**：非技术用户体验验收 + 最小热修  
**先等**：无（W1 UI 已宣称 ✅）  
**可并行**：B C D  

**改哪里（热修才动）**  
- 优先只读/点：`dist/CCO.app` 或 dev 桌面  
- 热修 scope（仅当抽检失败）：`web/js/features/chat/chatUnderstand.js` · `chatRender.js` · `chatPersona.js` · 相关 css ** narrowly**  
- **禁止**：动 `chatPathMode` 大逻辑、Rust、拆分台、W2  

**提示词（整段粘贴）**

```text
你是 cco 桌面体验验收窗口 A（W1-6）。

【目标】
按非技术用户走「含糊三轮边聊」，确认：主输入好找、无三档英雄键、当前理解条更新、按我说的改/这版作数/去拆步可用，且不逼满页 1/5 问卷；不出现 L/M/H、P1–P6、引擎黑话第一句。

【必读】
- docs/path-depth-wave-2026-07-28/landing.md（W1-6、词汇表）
- docs/path-depth-wave-2026-07-28/02-iterate-clarity.md
- docs/path-depth-wave-2026-07-28/03-path-empty-cta.md（空态目标：输入+芯片，无三键）

【步骤】
1. 启动桌面（优先已有 dev/打包方式；不要大改打包脚本）。
2. 空态：是否主输入第一？有无「快试|一份计划|多需求」三英雄键？（应无）
3. 点 2 个不同场景芯片：开场占位/主 CTA 是否可分辨？
4. 含糊输入三轮（例：「我想做个能收客资的小东西」→ 补充给谁 → 再改不做支付）：
   - 「当前理解」是否更新？
   - 假设是否像假设而非「你已确认」？
5. 出草稿后：唯一主 CTA、直接执行是否降权、有无双卡同文。
6. 点「按我说的改」「这版作数」：是否开跑？（禁止开跑）
7. 将结果写成简短 CHECKLIST（过/不过+截图或步骤）；仅对「不过」做最小热修。
8. 回写 docs/path-depth-wave-2026-07-28/landing.md 的 W1-6 ☐/✅ 与一句证据。

【怎样算做完】
- [ ] 抽检记录落盘（可写在 landing 或 .cco-out 短文）
- [ ] W1-6 勾选与证据
- [ ] 若有热修：diff 仅 chat 窄文件；无 confirm 旁路

【不要做什么】
重做 W0；实现 W2 Bundle；暴露 delivery/P 代号；扩大重构。
```

---

### 窗口 B · W2-4 supersede 按 plan_path

**角色**：Rust 计划 job 隔离  
**先等**：无  
**可并行**：A C D  
**改哪里**  
- `src/plan/planner/job.rs`（`supersede_planning_jobs` 等）  
- 相关测：`tests/` 或 job 单测  
- 契约若需一行：`docs/contracts/plan-job.md`  
**禁止**：大改 web UI；动 Scheduler 主循环无关逻辑  

**提示词**

```text
你是 cco 窗口 B：实现 W2-4 supersede 按 plan_path 隔离。

【目标】
同项目多份计划文档可各自 planning/planned：开始 plan_path=A 的 job 时，只 supersede/取消「同 project + 同 plan_path」的其它 planning job，不得误杀 plan_path=B 的 planned/planning。

【必读】
- docs/path-depth-wave-2026-07-28/landing.md W2-4
- docs/path-depth-wave-2026-07-28/04-multi-plan-bundle.md
- docs/contracts/plan-job.md
- src/plan/planner/job.rs（supersede_planning_jobs 现状）

【步骤】
1. 读现 supersede 条件（是否整 project）。
2. 改为：匹配 project + plan_path（路径规范化一致）才 cancel/supersede。
3. planned / confirmed 的其它 path 一律保留。
4. 单测至少：
   - 同 project 不同 plan_path：新 A 不杀 B planning/planned
   - 同 plan_path：新 job 仍 supersede 旧 planning
5. 若契约文需改，只改 plan-job 相关句，不写平行阶段表。
6. cargo test 相关用例绿。

【怎样算做完】
- [ ] 行为符合上表
- [ ] 测绿
- [ ] landing W2-4 可勾（由关账窗或你勾并写证据）

【不要做什么】
实现完整 Bundle UI；改 confirm 入口；教用户 L/M/H。
```

---

### 窗口 C · W2 域模型骨架（Bundle / Index）

**角色**：domain + app 薄层，为 E/F 铺路  
**先等**：无（可与 B 并行；合并时注意）  
**可并行**：A B D  
**改哪里**  
- 新建宜：`src/domain/` 下 thin 模块（如 plan bundle 或 split 旁）  
- `src/app/` 用例骨架 free-fn 或小模块  
- **禁止**先堆 `services` 上帝；禁止 Domain→UI  
- 可选：`docs/path-depth-wave-2026-07-28/04` 补字段冻结一小段（不新开阶段表）  

**提示词**

```text
你是 cco 窗口 C：落地 W2 的 WorkBundle / 索引 薄域模型与 app 骨架（尚不必完整 UI）。

【目标】
定义并对内可用：
- WorkBundle { id, project, plans[], edges? }
- PlanRef { plan_path, job_id?, status?, task_count? }
- PlanEdge 可选 { from, to, kind: after | parallel_ok }
- Index 文档约定：人话章节（本波要什么/材料关系/计划列表/可并必串/不做/齐活）——可先常量/模板字符串，不强制 LLM。

app 至少提供可测的纯/IO 边界草案其一：
- create_bundle_from_paths(project, plan_paths[])
- 或 record index path + member plans
不实现完整「索引 LLM 生成」也可，但接口与类型要稳，单测覆盖序列化/校验。

【必读】
- docs/path-depth-wave-2026-07-28/04-multi-plan-bundle.md
- docs/path-depth-wave-2026-07-28/landing.md W2
- src/domain/CLAUDE.md · src/app/CLAUDE.md
- 工程硬规则：确认仍唯一开跑；认领索引 ≠ 开跑

【步骤】
1. 选型最小类型放 domain（无 Tauri/clap）。
2. app 暴露 create/list/get 级薄函数（可暂存 JSON 于 state_root 子目录，路径写清）。
3. 校验：空 plans 拒绝；edge 端点必须在 plans 内。
4. 单测绿；文件行数守硬顶。
5. 在 04 文或 landing 补一行「类型锚点 path」。

【怎样算做完】
- [ ] 类型 + 最少 app API + 测
- [ ] 文档锚点
- [ ] 无 UI 大改；无 confirm 旁路

【不要做什么】
拍扁多计划成一个超级 PlanIR；实现甘特；暴露 Bundle 工程名作主文案第一句。
```

---

### 窗口 D · 文档薄关账（PRODUCT / 索引）

**角色**：纯文档  
**先等**：无  
**可并行**：A B C  
**改哪里**  
- `PRODUCT.md` 受众表扩或链到 docs/…/01-audience.md  
- `docs/CLAUDE.md` 若索引行过时则一行更新  
- **禁止**改业务代码  

**提示词**

```text
你是 cco 窗口 D：文档薄关账（W4-1/W4-4 可先做部分）。

【目标】
1. PRODUCT.md「给谁用」与 docs/path-depth-wave-2026-07-28/01-audience.md 对齐或显式链接；不改五步主循环顺序。
2. 确认 docs/CLAUDE.md 活跃区有 path-depth-wave-2026-07-28 索引。
3. 全文不把 L/M/H 当用户主路径推荐；若 PRODUCT 被误写三档，改为人话场景。

【必读】01-audience.md · 01b 开头 PROTOCOL · landing 词汇表

【怎样算做完】
- [ ] PRODUCT 有扩表或「详见 01」链接
- [ ] docs/CLAUDE 索引正确
- [ ] 无新阶段勾选表

【不要做什么】
改 web/rust；开 W2 功能。
```

---

## 波次 β · 三窗（B✅ 且 C 类型可用后）

---

### 窗口 E · 需求包 → 索引 → N 计划（W2-1…W2-3）

**先等**：C 的 API/类型；B 建议已合（多 path 拆才安全）  
**改哪里**：chat 多附件约定 / plan 落盘 / 接 ModelSplit 前的「多 plan 文件」生成（提示词+保存）；`docs/runtime-prompts` 可补索引结构短节  
**禁止**：改 supersede 再打架（B 已做）  

**提示词**

```text
你是 cco 窗口 E：W2-1…W2-3 需求包入场 → 索引 md 认领 → 生成 N 份执行计划 md。

【目标】
用户丢入多份材料或点「大促多页」类场景后：
1) 可生成/保存《本波索引》md（人话结构见 04）
2) 认领索引 ≠ 开跑
3) 从索引切出 ≤5–7 份执行计划文件（切割启发式见 04）
4) 进入方式：芯片/话术/附件推断 bundle，禁止 UI「请选 H/多需求档」

【必读】04-multi-plan-bundle.md · 03 去三档 · landing W2 · 窗口 C 的类型锚点

【步骤】
1. 对齐 C 的 Bundle API；没有则先最小接盘。
2. 索引模板 + 保存路径约定（项目 plans/ 或 state）。
3. 多 plan 落盘命名稳定（可测）。
4. 提示词：禁止一篇万字冒充多交付。
5. 测或脚本：两材料 → 索引 → ≥2 plan 路径。

【怎样算做完】
- [ ] 两 plan 路径可复现
- [ ] 认领索引不调用 confirm_start
- [ ] landing W2-1…3 证据

【不要做什么】
自动连开多 run；超级 Task 大图。
```

---

### 窗口 F · 分计划拆分隔离与重拆（W2-5）

**先等**：B（supersede path）✅；E 有多 plan 或可用夹具多 path  
**改哪里**：`src/app/split.rs` / planner start 多 job；桌面「只重拆这一份」若有入口  
**提示词**

```text
你是 cco 窗口 F：W2-5 分计划排队/隔离拆分 + 单计划失败可重拆。

【目标】
Bundle 内 plan A/B：A 拆失败不影响 B 的 planned；可对失败 path 带 revision 重拆；默认排队拆（可先串行 start_plan_job per path）。

【必读】05-modular-split-run.md · landing W2-5 · job supersede（B 的行为）

【步骤】
1. 串行或限流对 bundle.plans[] 调现有 start_plan_job。
2. 汇总每 path 状态到 Bundle 视图模型（可先 CLI/DTO）。
3. 失败 path 重拆 API：不 reset 成功 path。
4. 测：A fail B planned 仍在。

【怎样算做完】
- [ ] 隔离测绿
- [ ] 无整包 plan_failed 误杀

【不要做什么】
重写 Scheduler；默认同 worktree 多计划真并行写码。
```

---

### 窗口 G · Bundle 总览 DTO + 确认语义（W3-1…W3-2）

**先等**：C；有假数据也可先 UI  
**改哪里**：`src/app` 查询 DTO；`web/js/features/split` 或新 `bundle` 薄模块；gateway 一端  
**提示词**

```text
你是 cco 窗口 G：W3-1 总览 DTO + W3-2 Bundle 确认（多份仍走 confirm 语义）。

【目标】
人话总览：几份计划、每份任务数、波次/可并必串、模型分工人话（复用 cost-route 展示，不堆引擎名）。
确认本波 = 对每份 planned 调用既有 confirm（可批触发），禁止 start_run 旁路；optional 不静默。

【必读】05 · landing W3 · split-product-rules · cost-aware 人话条

【步骤】
1. DTO 字段冻结（人话优先）。
2. gateway + 最小总览条/卡片（概念≤3：几份计划·能否一起干·确认本波）。
3. confirm 循环接 app::split::confirm。
4. 测：mock 两 job planned → 两次 confirm 语义。

【怎样算做完】
- [ ] DTO + 最小 UI 或 CLI 打印
- [ ] 无旁路开跑

【不要做什么】
甘特；主路径 provider 列表。
```

---

## 波次 γ · 两窗

---

### 窗口 H · 执行默认与巡检（W3-3…W3-4）

**先等**：G 确认链；F 多 job  
**提示词**

```text
你是 cco 窗口 H：W3-3 默认计划串行执行 + 高级并行门闩；W3-4 per-plan 巡检入口 + 本波齐活小结。

【目标】
多计划执行默认按 edge/列表串行开跑；仅声明 parallel_ok 且隔离策略满足才并行（可先只做串行+TODO 门闩）。
结果台/入口可按 plan 看巡检；本波给一句「齐/不齐」人话。

【必读】05 · multi-cli 同项目多 run 慎用 · inspect/Ensure 既有能力

【不要做什么】
同 worktree 默认真并行抢写；重写巡检引擎。
```

---

### 窗口 I · W4 关账

**先等**：α 完成；β 主路径可演示或明确 residual  
**提示词**

```text
你是 cco 窗口 I：W4 关账。

【目标】
- W4-1 文案抽检 01/01b
- W4-2 金样：单计划 + 两计划失败隔离（文案勿 L/M/H）
- W4-3 scripts/check-arch.sh 与行数
- W4-4 PRODUCT（若 D 未做则补）
- W4-5 landing 状态行收口；主路径 rg 无新三档课/P 代号教用户

【怎样算做完】landing W4 全 ✅ 或明确 residual 单列。
```

---

## 1. 合并与冲突预防

| 风险 | 规则 |
|------|------|
| A 与 G 都改 chat/split UI | α 时 A 只热修；G 等 β |
| B 与 F 都改 job | F 基于 B 合并后分支 |
| C 与 E 都建 Bundle | E 必须接 C 锚点，禁止第二套类型 |
| 多窗同时改 landing | **只允许**改自己的 ☐ 行；冲突时保留双方 ✅ 证据 |

## 2. 每窗交付格式（统一）

```text
## 窗口 X 小结
- 做了：
- 测/证据：
- 未做/residual：
- landing 勾选建议：
```

---

## 修订

| 日期 | 说明 |
|------|------|
| 2026-07-28 | 初版：按 landing 余量拆 αβγ 多窗提示词 |
