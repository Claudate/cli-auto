# 本波执行 · 完整多窗口提示词（2026-07-28 现况）

> **勾选真源**：[docs/path-depth-wave-2026-07-28/landing.md](../../docs/path-depth-wave-2026-07-28/landing.md)  
> **索引**：[docs/path-depth-wave-2026-07-28/INDEX.md](../../docs/path-depth-wave-2026-07-28/INDEX.md)  
> **真人抽检表**：[docs/path-depth-wave-2026-07-28/w1-6-desktop-checklist.md](../../docs/path-depth-wave-2026-07-28/w1-6-desktop-checklist.md)  
> **行数债**：[docs/path-depth-wave-2026-07-28/w4-3-line-debt.md](../../docs/path-depth-wave-2026-07-28/w4-3-line-debt.md)  
> **结构冒烟**：`node scripts/path-depth-wave-smoke.mjs`（宣称 34/34）

---

## 0. 先读：现在还剩什么

### 0.1 主路径状态（不要重复实现）

| 波次 | 状态 | 含义 |
|------|------|------|
| **W0** 空态/芯片/去三档/草稿 CTA | ✅ | 禁止再加「快试\|一份计划\|多需求」三英雄键 |
| **W1-0…W1-5** 当前理解/这版作数/反馈/prompt | ✅ | 禁止当零起点重做 |
| **W1-6** 真人桌面抽检 | ☐ **唯一正式 residual** | 清单已备；结构脚本不可代关账 |
| **W2** 本波认领/分组/supersede per path | ✅ | |
| **W3** 总览/批确认串行/齐活一句 | ✅ | 真并行门闩明确 **不做** |
| **W4** 文档关账/金样/债记账 | ✅ | clarify 巨石等另立，不回灌本波 |

### 0.2 词汇（全窗统一 · 零歧义）

| 词 | 用户可见？ | 说明 |
|----|------------|------|
| 场景芯片 | 是 | 验证想法 / 上架 / 大促多页… |
| 画像 | 否 | 01b 旋钮，无代号 |
| `delivery` = trial \| single \| bundle | **否** | 只表现为行为；**废名 L/M/H** |
| P1–P6 | 否 | 内部透镜 |
| 本波 / 索引 / 认领本波 | 是（人话） | 认领 ≠ 开跑 |
| confirm / 确认本波 | 是 | **唯一**开跑语义 |

### 0.3 全窗红线

1. **开跑**只经 `confirm_start` / Split.confirm（批确认也必须走同一闸）。  
2. 认领本波、保存、这版作数、拆成步骤、按我说的改 → **禁止** spawn 业务 worker。  
3. 主路径第一句禁止：run_id、VERDICT、provider 内部名、P1–P6、L/M/H、三档课。  
4. 不新建上帝 `*Manager`；不往 `state.js` / 厚 facade 堆策略。  
5. 业务文件软 400 / 硬 600；本波不强拆历史巨石（见 w4-3），**新增**勿再胀 hard。  
6. CLI 与桌面同一 app 路径；IPC 只 `gateway`。  
7. 地图与地形：改行为先改 landing/对应 docs，再改代码。

### 0.4 推荐开窗策略（现况）

```text
必开（关本波成功标准 residual）
  窗 V-HUMAN …… W1-6 真人抽检（可只开 1 人）

建议并行加固（主路径已 ✅，属质量/债，不改 landing 主叙事除非修 bug）
  窗 V-REGRESS … 回归冒烟 + 边界金样复跑
  窗 V-COPY …… 文案/禁词扫尾（L/M/H、P 代号、04 旧表）
  窗 V-DEBT …… clarify 巨石纵切预研或第一刀（另立债，可标非本波）
  窗 V-POST …… landing「后置」三项之一：项目级记住芯片 / grain·clarify 进 prompt

禁止
  再开「从零实现 W2 Bundle 类型」类窗（已落）
  无隔离多计划真并行写码
```

---

# 第一部分 · 必开窗（完整提示词）

---

## 窗 V-HUMAN · W1-6 真人桌面抽检关账

### 元信息

| 项 | 内容 |
|----|------|
| **目标** | 把 landing **W1-6** 与成功标准「边聊 ≥3 轮口径可指认」用**真人**关账（或明确 residual） |
| **依赖** | 无代码依赖；需可开桌面 |
| **并行** | 可与 V-REGRESS / V-COPY 并行；**不要**同时大改 chat UI |
| **主读** | `w1-6-desktop-checklist.md` · `landing.md` · `02` · `03` |
| **可写** | 清单记录区；landing W1-6 一行；抽检失败时的**最小**热修 |
| **禁写** | W2/W3 大功能；Scheduler；新三档 UI |

### 完整提示词（整段复制到新 Agent 会话）

```text
# 角色
你是 cco 本波「窗 V-HUMAN」：负责 W1-6 真人（或等效录屏）桌面抽检。你不是从零实现功能，主路径 W0–W3 已宣称完成。

# 仓库与真源
- 工作根：本仓 claude-auto（cco）
- 勾选只认：docs/path-depth-wave-2026-07-28/landing.md
- 抽检表：docs/path-depth-wave-2026-07-28/w1-6-desktop-checklist.md
- 空态目标：docs/path-depth-wave-2026-07-28/03-path-empty-cta.md
- 边聊目标：docs/path-depth-wave-2026-07-28/02-iterate-clarity.md
- 词汇：landing「词汇（执行无歧义）」——禁止教用户 L/M/H 与 P1–P6

# 背景（勿重复实现）
已完成：去三档英雄键、场景芯片画像、当前理解条、这版作数、按我说的改、wave-index 认领本波、计划管理本波分组、总览、批确认串行 confirm_start、supersede 按 plan_path。
结构冒烟：node scripts/path-depth-wave-smoke.mjs（可先跑，**不能**代替本窗关 W1-6）。

# 你的唯一完成定义
1. 按 w1-6-desktop-checklist.md 的 A/B/C/D 表逐项勾选（过/不过 + 一句现象）。
2. 在清单「记录」区写：环境（dev/打包）、日期、操作者、结构冒烟是否 PASS、真人结论。
3. 若全过：landing.md 将 W1-6 与成功标准「边聊≥3轮」标 ✅，状态行去掉「真人 ☐」。
4. 若不过：landing 保持 ☐，写清 residual（哪一格、严重度、是否需热修）；对 P0 断主路径的 bug 才做最小热修。

# 操作步骤（必须按序）

## 0. 准备
- 打开桌面：优先已有 dist/CCO.app 或项目文档中的 dev 启动方式；不要为了抽检重写打包脚本。
- 选空项目或临时目录；新开聊天会话。
- 可选先跑：
  node scripts/path-depth-wave-smoke.mjs
  node scripts/claim-boundary-check.mjs
  记录 PASS/FAIL，仍继续真人 A–D。

## 1. 区块 A · 空态（W0-8）
对清单 A1–A5 逐项：
- A1 无「快试 / 一份计划 / 多需求」三英雄键
- A2 主输入视觉第一
- A3 芯片「上架详情」口吻可辨
- A4 芯片「制度发文」与上架可区分
- A5 高级折默认不必点
全部填 过？列。

## 2. 区块 B · 含糊三轮（W1 核心）
固定话术（可微调但须含糊→补给谁→补不做）：
  轮1：「想做个给客户看的东西，还没想清」
  轮2：「主要给销售用」
  轮3：「先不做登录和支付」
检查 B1–B5：非满屏 1/5 大门；当前理解更新给谁/不做；假设不装已确认；无 P/L/M/H/run_id 第一句；「按我说的改」不开跑。

## 3. 区块 C · 本波多计划（W2–W3 冒烟）
话术：「本波要日语落地页和英语落地页两件，一起排。」
检查 C1–C6：多计划或 wave-index/认领条；认领 toast 含未开跑；计划管理本波分组；INDEX 不能拆步；总览人话；只拆 A 时 B 仍在；确认本波走闸且同仓串行提示。

## 4. 区块 D · 红线
D1–D3：认领/保存/拆步未静默开跑；开跑只在确认；optional 未静默勾上。

## 5. 热修规则（仅失败时）
允许改的窄范围（一次尽量一个主题）：
- web/js/features/chat/chatUnderstand.js
- web/js/features/chat/chatPersona.js
- web/js/features/chat/chatRender.js（空态/条）
- web/js/features/chat/chatFormat.js（认领条文案）
- 相关 CSS 仅改可见 bug
禁止：chatClarify 大拆、Rust 调度、新功能、加回三档键。
热修后重跑失败项 + path-depth-wave-smoke。

## 6. 回写
- 更新 w1-6-desktop-checklist.md 记录区
- 更新 landing.md W1-6 与成功标准对应行、状态行
- 输出「窗 V-HUMAN 小结」（见文末统一格式）

# 怎样算做完（验收清单）
- [ ] A/B/C/D 表每格有 过/不过
- [ ] 记录区含环境与结论
- [ ] landing W1-6 与「边聊≥3轮」已 ✅ 或 residual 写清
- [ ] 无 confirm 旁路；无新三档 UI
- [ ] 若有代码：测/冒烟相关绿；diff 范围窄

# 明确不要做什么
- 不要宣称「脚本 PASS = W1-6 ✅」
- 不要实现无隔离多计划真并行
- 不要重做 W2 认领/总览
- 不要把 P1–P6 写进用户可见文案
- 不要新开平行阶段勾选表

# 交付格式
## 窗 V-HUMAN 小结
- 环境：
- 结构冒烟：
- A/B/C/D：通过数 / 失败项
- 热修：无 | 文件列表
- landing：W1-6 ✅ / residual …
- 风险：
```

---

# 第二部分 · 建议并行加固窗（完整提示词）

> 主路径已 ✅。下列窗用于**质量、禁词、债、后置**；改 landing 仅当修 bug 或关 residual。

---

## 窗 V-REGRESS · 自动化回归与边界金样

### 元信息

| 项 | 内容 |
|----|------|
| **目标** | 复跑本波相关测与冒烟，抓住回归；输出证据表 |
| **依赖** | 无 |
| **并行** | ∥ V-HUMAN（勿同时大改同文件）、∥ V-COPY |
| **主改** | 仅修复失败测试的最小 diff |
| **禁** | 新功能范围蔓延 |

### 完整提示词

```text
# 角色
你是 cco「窗 V-REGRESS」：本波 path-depth / wave 相关自动化回归守卫。不负责真人抽检关账（那是 V-HUMAN）。

# 真源
- docs/path-depth-wave-2026-07-28/landing.md（W2-4、W2 落盘、W4-2 金样描述）
- docs/contracts/plan-job.md（若涉及 supersede）
- scripts/path-depth-wave-smoke.mjs
- scripts/claim-boundary-check.mjs
- scripts/clarify-split-visual-smoke.mjs（若环境允许）
- src/plan/planner/job.rs 中 supersede_planning_is_per_plan_path 等测

# 红线
confirm 唯一开跑；认领≠开跑；supersede 按 plan_path；不教 L/M/H。

# 步骤
1. 跑并记录：
   node scripts/path-depth-wave-smoke.mjs
   node scripts/claim-boundary-check.mjs
   cargo test supersede_planning_is_per_plan_path -- --nocapture
   以及任何 chat_save_wave_bundle / wave 相关测（先 rg 测试名再跑，避免全仓过慢可先 filter）。
2. 可选：./scripts/check-arch.sh（期望 FAIL=0；WARN 对照 w4-3-line-debt.md，本窗不强拆巨石）。
3. 失败则：定位最小根因 → 最小修复 → 重跑失败项。
4. 写证据：命令、exit code、关键断言名；可追加到 landing 修订或 .cco-out 短文（若仓有惯例）。
5. 若发现「文档宣称 ✅ 但测红」：在 landing 状态行加 residual 一句，勿默默改宣称而不修。

# 完成定义
- [ ] 上列脚本/关键测结果表（绿或已修绿）
- [ ] 若有修：diff 说明 + 重跑证据
- [ ] 未引入三档 UI / 开跑旁路

# 不要做
全量无关 refactor；STRICT=1 强拆历史 hard 债；改 PRODUCT 大方向。

# 交付
## 窗 V-REGRESS 小结
- 命令与结果表
- 修复文件
- residual
```

---

## 窗 V-COPY · 禁词与文档歧义扫尾

### 元信息

| 项 | 内容 |
|----|------|
| **目标** | 用户可见路径与本波 docs **无** L/M/H 课、无 P 代号教用户；修 04 等残留「单计划 M / 多计划 H」表 |
| **依赖** | 无 |
| **并行** | ∥ 全窗（纯文案/docs 为主） |
| **可写** | docs/path-depth-wave-2026-07-28/* · web 用户可见字符串（chat 文案） |
| **禁** | 大改逻辑 |

### 完整提示词

```text
# 角色
你是 cco「窗 V-COPY」：禁词与表述消歧。产品已去三档；文档/UI 字符串若仍教 L/M/H 或「选 H」会误导执行者与用户。

# 真源词汇
landing.md 词汇表：
- 对外：场景芯片、本波、认领、拆成步骤、确认
- 对内：delivery trial|single|bundle；P1–P6
- 废名：L/M/H（仅 03 迁移对照可出现「曾称」）

# 步骤
1. 在下列范围搜索并分类（保留「禁止/废名/曾称」语境，改「当作主路径推荐」语境）：
   docs/path-depth-wave-2026-07-28/
   plans/wave-2026-07-28/
   web/js/features/chat/（用户可见中文串）
   docs/runtime-prompts/chat-plan-writing.md（若仍教三档）
2. 必修例：docs/.../04-multi-plan-bundle.md 中「单计划 M | 多计划 H」表 → 改为「单计划 | 本波多计划」人话，去掉 M/H 字母。
3. 用户可见 UI：不得出现「请选择 L/M/H」「作业模式 P3」等。
4. 对内代码标识符 delivery/trial 可保留；注释勿写「用户选 H」。
5. 出对照表：文件 | 旧句 | 新句。

# 完成定义
- [ ] 04 等主路径 docs 无「当作推荐」的 L/M/H 分档表
- [ ] chat 用户串抽检无 P 代号/三档课
- [ ] 变更列表

# 不要做
重写整份 06 田野长文非歧义段落；改 Rust 算法。

# 交付
## 窗 V-COPY 小结
- 修改文件列表
- 仍允许残留的「废名/禁止」句位置
```

---

## 窗 V-POST · landing 后置三项（选一或拆子任务）

landing W0 完成段写明后置：

1. clarify_depth / grain 进 prompt  
2. 项目级记住场景芯片  
3. （bundle「真引擎」大部已有；勿重复认领——若做则只补缺口）

### 完整提示词（默认做 1+2 的最小集）

```text
# 角色
你是 cco「窗 V-POST」：消化 path-depth 本波 landing 写明的**后置**薄项，不新开阶段表。

# 范围（只做下列，做完即停）
P0-A：01b 的 clarify_depth、split_grain（或 grain_hint）在写作/拆分提示或 start_plan_job 请求链路上有合理默认（随画像/芯片），且用户不可见内部名。
P0-B：场景芯片选择可按项目记住（local 或已有 project UI state），下次打开同项目恢复；可跳过、可改。

# 必读
- docs/path-depth-wave-2026-07-28/01b-persona-path-profiles.md（旋钮）
- docs/path-depth-wave-2026-07-28/landing.md W0 后置句
- web/js/features/chat/chatPersona.js
- src/plan/planner 若已有 grain_hint
- docs/runtime-prompts/chat-plan-writing.md

# 红线
不恢复三档英雄键；不把 delivery 画成三按钮；confirm 不旁路；文件行数硬顶。

# 步骤
1. 读 chatPersona 现状：芯片是否已写 state、是否持久化到项目。
2. P0-B：若无持久化，用现有 project/session 存储惯例最小接入；读写圆通；无项目时不报错。
3. P0-A：画像 clarify_depth → 影响是否默认展开澄清/软问深度（可与现 chatClarify 协作，禁止再堆 chatClarify 千行大补丁——宁可用参数/小模块）。
4. grain：芯片或 delivery_bias 映射到拆分 grain_hint（若 start_plan_job 已支持）；单测或轻测。
5. 更新 landing「后置」句：已做/未做划清。

# 完成定义
- [ ] 同项目重开芯片仍在（或文档说明限制）
- [ ] clarify/grain 有可描述行为差（两芯片对比）
- [ ] 无新三档 UI
- [ ] 测或手测步骤写下

# 不要做
W3 真并行门闩；人生 Pack；guided G 全量；拆 chatClarify 全文件（那是 V-DEBT）。

# 交付
## 窗 V-POST 小结
- 实现了 P0-A/B 哪些
- 关键文件
- landing 后置句如何改
```

---

## 窗 V-DEBT · chatClarify 巨石第一刀（可选 · 标非本波主路径）

### 元信息

| 项 | 内容 |
|----|------|
| **目标** | 按 w4-3 建议，将 `chatClarify.js` 按 phase 纵切第一刀，降 hard 风险 |
| **依赖** | 无；与 V-HUMAN 错开改同一 UX |
| **注意** | **不是** landing W0–W4 未完成项；属债。做完更新 w4-3，勿假装本波新阶段 |

### 完整提示词

```text
# 角色
你是 cco「窗 V-DEBT」：处理 w4-3-line-debt 中 chatClarify.js 历史硬债的**第一刀纵切**，服务可维护性，不改变产品主叙事。

# 必读
- docs/path-depth-wave-2026-07-28/w4-3-line-debt.md
- web/js/features/chat/chatClarify.js（先读结构与 export）
- web/js/features/chat/installChat.js / chatRender 引用方式
- scripts/check-arch.sh 规则
- L1 CLAUDE.md 体积硬规则

# 目标
1. 在不改外部行为的前提下，把 chatClarify.js 拆出 ≥1 个内聚模块（建议：empty 渲染 / 选项卡 / brief 认领 三选一先拆最热）。
2. 原文件变 facade re-export 或变薄，行数明显下降。
3. 现有 clarify 相关冒烟/测仍绿（clarify-split-visual-smoke 或仓内 clarify 测）。

# 步骤
1. 画依赖：谁 import chatClarify 的哪些 export。
2. 选切口：按 DOM 区块或按函数簇，避免循环依赖。
3. 搬移 + 更新 import；不改文案语义。
4. 跑相关冒烟；记录行数前后。
5. 更新 w4-3-line-debt.md 本波相关前端表。

# 完成定义
- [ ] 新文件 + 旧文件变薄有数字
- [ ] 行为目测/冒烟不差
- [ ] w4-3 已记一刀

# 不要做
借机重做澄清产品；加三档；改 Rust；一次拆完 2500 行（第一刀即可）。

# 交付
## 窗 V-DEBT 小结
- 前后行数
- 新模块路径
- 冒烟结果
```

---

## 窗 V-WAVE-HARDEN · 本波多计划路径加固（可选）

若你认为 W2/W3「薄实现」不够稳，用本窗**加固**而非重写。

```text
# 角色
你是 cco「窗 V-WAVE-HARDEN」：在已有 wave 认领/分组/总览/批确认之上做加固，不推翻架构。

# 已有锚点（先 rg 再改）
- web/js/features/chat/chatWavePlans.js
- web/js/features/chat/chatWaveOverview.js
- web/js/features/chat/chatWaveBatch.js
- web/js/features/chat/chatFormat.js（wave-index / 认领条）
- save_wave_bundle / chat_save_wave_bundle 测
- src/plan/planner/job.rs supersede per plan_path

# 可选加固包（按痛选，做 1～2 项即停）
1. 批确认：活跃 run 时文案与按钮 disable 更清晰；失败中断后续 path 的提示。
2. INDEX 误点拆步：更死人话 toast。
3. 总览 closeout 与真实 job 状态不同步时的刷新。
4. 多 plan 落盘失败时部分成功的回滚或人话列表。
5. 单测补：认领不碰 confirm_start（若尚无）。

# 红线
批确认必须仍循环 confirm_start；禁止 start_run 旁路；禁止同仓默认真并行多 run 写码；禁止 L/M/H UI。

# 完成定义
- [ ] 所选加固项有测或手测步骤
- [ ] smoke 仍绿
- [ ] landing 仅当修 bug 时加一句证据，不新开 W5 表

# 交付
## 窗 V-WAVE-HARDEN 小结
- 选做项
- 文件
- 证据
```

---

# 第三部分 · 统一纪律

## 合并冲突

| 区域 | 谁优先 |
|------|--------|
| chatUnderstand / 空态渲染 | V-HUMAN 热修 > 他人 |
| chatClarify | 仅 V-DEBT |
| chatWave* | 仅 V-WAVE-HARDEN |
| docs 禁词 | V-COPY |
| landing 勾选 | 各窗只改自己负责的一行 |

## 每窗结束必须交的小结模板

```markdown
## 窗 <ID> 小结
- 目标对齐 landing 哪一行：
- 做了：
- 证据（测/脚本/清单格）：
- 代码文件：
- 未做 / residual：
- 是否动到 confirm 路径：是/否（否才合格，除非 G/W3 批确认且仍走 confirm_start）
- 风险：
```

## 给你的调度口令（现况）

```text
1. 必开：V-HUMAN（关 W1-6）
2. 同时可开：V-REGRESS + V-COPY
3. 有余力：V-POST（芯片记忆 + grain）或 V-DEBT（clarify 第一刀）
4. 觉 wave 不稳：V-WAVE-HARDEN（选 1～2 痛点）
5. 全部结束后：人工扫 landing 状态行与成功标准，只留真实 residual
```

---

## 修订

| 日期 | 说明 |
|------|------|
| 2026-07-28 | 初版 EXEC-multi-window（按当时未做 W2 假设） |
| 2026-07-28 | **完整版**：按 W0–W4 主路径已 ✅ 重写；必开 V-HUMAN + 并行加固全量提示词 |
