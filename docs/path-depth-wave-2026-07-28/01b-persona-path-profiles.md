# 计划 01b · 职业路径画像（多维默认，不是只贴 L/M/H）

> 本波：[INDEX.md](./INDEX.md) · 父篇：[01-audience.md](./01-audience.md) · 田野：[06-audience-demand-field-research.md](./06-audience-demand-field-research.md)  
> 问题：用户批评「职业 × 路径」仍只有 L/M/H 三格——**没有按人群优化**。本文补齐。  
> 状态：决策向 · W0 文案/模板/默认旋钮真源  

[PROTOCOL]: **L/M/H 是工作方式骨架（共享引擎）**；职业优化落在 **画像旋钮**（开场、产物、闸、验收语言、并行食欲…）。禁止为每职业复制三套 App。禁止进门强制选职业；画像来自：用户自选场景芯片 / 项目类型 / 上次选择 / 可跳过。改默认先改本文再改 UI/prompt。

---

## 0. 为什么「只写 L/M/H」等于没优化

| 错法 | 问题 |
|------|------|
| 电商→L，PM→M，大促→H | 同一字母下，**开场、验收、风险、按钮文案全一样** → 用户无感 |
| 三档当唯一差异 | 行政的「稳」和卖家的「快」都可能是 M，但 **确认严格度、直接执行、验收词** 应完全不同 |
| 职业锁死路径 | 小老板也能 H；PM 也能 L 试一句 → 锁死=假优化 |

**正确结构**：

```text
L/M/H          = 本波交付形态（几份计划、深浅）
PersonaProfile = 九类旋钮默认（文案·闸·产物·跑法·验收语言）
P1–P6          = 此刻作业模式（可覆盖画像默认）
```

同一职业可 L 也可 H；**变的是默认旋钮与例子，不是另做产品。**

---

## 1. 旋钮清单（实现时配置化，勿写死 if 职业）

| 旋钮 ID | 含义 | 取值例 |
|---------|------|--------|
| `path_bias` | 空态推荐的工作方式 | L / M / H（可一主一副） |
| `p_mode_bias` | 更常的作业模式 | P1…P6 主+次 |
| `opener` | 输入框占位 + 副文案 | 人话一句 |
| `example_chips` | 空态可点例子（≥2） | 职业场景 |
| `clarify_depth` | 默认盘问深度 | `none` / `soft1` / `soft2` / `full_opt` |
| `artifact` | 默认产物形态 | `demo_short` / `single_plan` / `index_multi` |
| `primary_cta` | 草稿主按钮文案 | 「生成并看看」/「拆成步骤」/「生成本波目录」… |
| `direct_exec` | 直接执行 | `offer` / `hide` / `danger_only` |
| `split_grain` | 拆分粗细暗示 | `coarse` / `balanced` / `fine` |
| `parallel_appetite` | 并行预期 | `serial_ok` / `task_parallel` / `plan_parallel_hint` |
| `confirm_tone` | 确认闸体感 | `light` / `normal` / `strict` |
| `cost_posture` | 费用姿态人话 | `cheap_first` / `balanced` / `quality_check` |
| `done_when_lexicon` | 验收用语域 | 见 §3 各行 |
| `non_goal_preset` | 默认「先不做」提示 | 职业常见砍项 |
| `template_keys` | 推荐模板 ID | 与 templates 目录对齐 |
| `progress_metaphor` | 进度话术 | 试做 / 立项 / 战役 / 上架 / 开课 / 发文… |

**覆盖序（高优先覆盖低）**：

```text
用户当场选择（L/M/H、芯片、明确「只要快试」）
  > 本项目已存 persona / 场景
  > 画像 path_bias 等默认
  > 产品全局默认（M + soft 可跳过澄清）
```

---

## 2. 九类主受众 · 完整画像（优化真源）

### 2.1 创业者 / 小老板 · `founder`

| 旋钮 | 默认 |
|------|------|
| path_bias | **L 主**，M 副（要给投资人/客户看完整说明时） |
| p_mode_bias | **P1** 主，P2 次 |
| opener | 「用一句话说：谁、有多痛、你想先做成哪一个能看的结果」 |
| example_chips | 报价单页可留资 · 周末验证小工具 · 客户能打开的介绍页 |
| clarify_depth | **none**（最多 soft1：给谁） |
| artifact | **demo_short** |
| primary_cta | **生成并看看**（L）/ 写清楚再拆步（转 M 时） |
| direct_exec | **offer**（短说明时）；长了自动建议拆步 |
| split_grain | **coarse**（少步、能演示） |
| parallel_appetite | task_parallel 可，不强调计划并行 |
| confirm_tone | **light**（仍要点确认，文案像「可以开始做了」） |
| cost_posture | **cheap_first** |
| done_when_lexicon | 能打开链接 · 主按钮可点 · 能演示给客户 3 分钟 |
| non_goal_preset | 先不做：支付/会员/复杂后台/多端 |
| progress_metaphor | **试做** |
| 沟通一句 | 「我们先做能给客户看的最小版本，别的写进不做。」 |

**与纯 L 标签差在哪**：CTA、砍项、验收词、费用姿态都按「老板周末验证」调，不是通用快试。

---

### 2.2 产品经理 · `pm`

| 旋钮 | 默认 |
|------|------|
| path_bias | **M 主**，H 副（需求包/多 epic 时抬升） |
| p_mode_bias | **P3+P4** 主，P6 强 |
| opener | 「要推进哪条需求？可粘 PRD/纪要；说清做成什么样、先不做啥」 |
| example_chips | 单需求可执行计划 · 多需求先做本波目录 · 对照验收清单 |
| clarify_depth | **soft2**（给谁 + 算完）；full 可折 |
| artifact | **single_plan**；材料多则 **index_multi** |
| primary_cta | **拆成步骤** / H 时 **生成本波目录** |
| direct_exec | **hide** 或 danger_only（整包当一步对 PM 几乎总是错） |
| split_grain | **balanced→fine**（可派工、有依赖） |
| parallel_appetite | **task_parallel**；H 时 **plan_parallel_hint** |
| confirm_tone | **normal**（立项感） |
| cost_posture | **balanced**；验收/整合 quality_check |
| done_when_lexicon | 验收条可勾 · 对照计划章节 · 遗漏可回补 |
| non_goal_preset | 先不做：范围外 epic、未定调研当实现 |
| progress_metaphor | **立项推进** |
| 沟通一句 | 「先收成可派工的计划，再拆开跑；做完按清单勾。」 |

**优化点**：不是「PM=M」，而是 **禁直接执行、细拆、对照验收词、H 入口文案=本波目录**。

---

### 2.3 设计师 / UI · `designer`

| 旋钮 | 默认 |
|------|------|
| path_bias | **M 主**，L 副（只出一屏草稿时） |
| p_mode_bias | **P3** 主，P6=「看得见」 |
| opener | 「第一眼用户看到什么？主按钮是什么？有没有不能动的视觉约束？」 |
| example_chips | 落地首屏三块 · 表单两步走查 · 组件说明可点 |
| clarify_depth | soft1（主按钮/受众） |
| artifact | single_plan；强调 **结构+文案+走查** 任务类型 |
| primary_cta | **拆成可预览的步骤** |
| direct_exec | hide（一锅炖易毁视觉） |
| split_grain | balanced；任务名偏 **版块/状态** 非工程模块 |
| parallel_appetite | 文案∥结构 task_parallel；谨慎同屏互改 |
| confirm_tone | normal |
| cost_posture | balanced；走查/预览偏 quality |
| done_when_lexicon | 预览像意图 · 主 CTA 位置对 · 关键态不崩 |
| non_goal_preset | 先不做：设计系统大一统、动效全集、多主题 |
| progress_metaphor | **出稿/走查** |
| 沟通一句 | 「先定第一眼和主按钮，再拆结构与走查。」 |

---

### 2.4 运营 / 增长 / 出海 · `ops`

| 旋钮 | 默认 |
|------|------|
| path_bias | **H 主**（多页/多语时），单活动页则 M |
| p_mode_bias | **P5** 主，P6=表单/线索/多语齐 |
| opener | 「本波要上几件事？截止日期？哪些页必须一起亮、哪些可后补？」 |
| example_chips | 大促三页本波 · 日/英落地+表单 · 活动页+线索表 |
| clarify_depth | soft2（截止 + 必上清单） |
| artifact | **index_multi** 优先于长单页 |
| primary_cta | **生成本波目录** → 再分计划拆 |
| direct_exec | hide |
| split_grain | balanced；**按页/语种切计划** |
| parallel_appetite | **plan_parallel_hint** + task_parallel（核心用户） |
| confirm_tone | normal；战役确认「本波可以开火」 |
| cost_posture | cheap_first 做页；终检 quality |
| done_when_lexicon | 页可开 · 表可收 · 语种齐 · 活动日状态可指认 |
| non_goal_preset | 先不做：年架重构、无关产品线 |
| progress_metaphor | **战役/本波** |
| 沟通一句 | 「先排本波几页谁先谁并，再拆每页步骤。」 |

**优化点**：默认就该像 **战役司令台**，不是「写一份计划的 PM 淡化版」。

---

### 2.5 新媒体 / 自媒体 / 内容 · `creator`

| 旋钮 | 默认 |
|------|------|
| path_bias | **L 主**，系列站 M |
| p_mode_bias | **P1+P3**（调性要改） |
| opener | 「链接要放哪、读者是谁、看完要他点什么？」 |
| example_chips | 简介铅封页 · 选题合集页 · 报名/加圈页 |
| clarify_depth | none / soft1（人设关键词） |
| artifact | demo_short |
| primary_cta | **生成并看看** |
| direct_exec | offer（真的很短时） |
| split_grain | coarse |
| parallel_appetite | serial_ok 为主 |
| confirm_tone | light |
| cost_posture | cheap_first |
| done_when_lexicon | 链接能发 · 像你的语气 · 主 CTA 可点 |
| non_goal_preset | 先不做：社区、复杂会员、全站 CMS |
| progress_metaphor | **上线发链接** |
| 沟通一句 | 「先做能放进简介的一页，语气不对再改一版。」 |

---

### 2.6 电商卖家 · `ecom`

| 旋钮 | 默认 |
|------|------|
| path_bias | **M 主**（单品完整上架），多 SKU/多活动 → H |
| p_mode_bias | **P3** 主（一致），P6=可上架 |
| opener | 「类目、卖点三句、必须出现的规格；禁止编造的参数有哪些？」 |
| example_chips | 单品详情可上架 · 活动页+券说明 · 多 SKU 分计划防串味 |
| clarify_depth | soft2（类目+禁编造） |
| artifact | single_plan；多品 **index_multi（一品一计划）** |
| primary_cta | **拆成上架步骤** |
| direct_exec | danger_only（详情结构一锅炖易翻车） |
| split_grain | balanced；模块=标题/卖点/规格/FAQ… |
| parallel_appetite | 多品 plan 级暗示串行或隔离并 |
| confirm_tone | normal |
| cost_posture | **cheap_first** 大段文案；合规自检 quality |
| done_when_lexicon | **上架清单**：必含词、模块齐、无违禁编造、主图位说明清 |
| non_goal_preset | 先不做：整站商城、支付中台 |
| progress_metaphor | **上架** |
| 沟通一句 | 「先锁卖点与禁编造，再拆模块；多品分开计划防串。」 |

**优化点**：验收词必须是 **上架清单**，不是「演示 3 分钟」；多品默认切计划——这才是电商优化，不是「也算 L」。

---

### 2.7 教师 / 培训 · `edu`

| 旋钮 | 默认 |
|------|------|
| path_bias | **M 主** |
| p_mode_bias | **P2**（边备边改），P6=学员路径 |
| opener | 「学员是谁、学完能做什么、作业/报名怎么交？」 |
| example_chips | 单课说明+作业 · 报名页 · 系列课目录 |
| clarify_depth | soft2（学员+学完标准） |
| artifact | single_plan（像薄教案） |
| primary_cta | **拆成开课步骤** |
| direct_exec | hide |
| split_grain | coarse–balanced；稳复用 |
| parallel_appetite | **serial_ok** 优先（稳>速） |
| confirm_tone | normal |
| cost_posture | balanced |
| done_when_lexicon | 学员打开不懵 · 交作业/报名通 · 下学期能改一章 |
| non_goal_preset | 先不做：直播中台、学分系统 |
| progress_metaphor | **开课** |
| 沟通一句 | 「按学员路径写：打开→学会→交作业。」 |

---

### 2.8 行政 / 财务 / HR · `admin`

| 旋钮 | 默认 |
|------|------|
| path_bias | **M 主**（极少 L；几乎不 H 除非多制度包） |
| p_mode_bias | **P3+P6**；怕乱改 |
| opener | 「文件给谁看、是否正式发布、哪几条表述不能 AI 擅改？」 |
| example_chips | 制度说明页 · 收集表+说明 · 内宣一页 |
| clarify_depth | soft2（受众+必审句） |
| artifact | single_plan |
| primary_cta | **生成草案并等人确认**（强调确认） |
| direct_exec | **hide** |
| split_grain | coarse；步骤含「人工审」optional |
| parallel_appetite | **serial_ok** |
| confirm_tone | **strict**（确认文案=发布前必看） |
| cost_posture | quality_check 表述；少自动 |
| done_when_lexicon | 可存档 · 流程可走 · 必审句未乱改 · 表可收 |
| non_goal_preset | 先不做：自动对外群发、改薪酬口径 |
| progress_metaphor | **发文/存档** |
| 沟通一句 | 「先出草案，你确认前系统不会当正式发布去跑。」 |

**优化点**：同为 M，与 PM 的差是 **strict 确认、禁直接执行、审句 non-goal、进度=发文**。

---

### 2.9 业务负责人（非开发）· `biz_lead`

| 旋钮 | 默认 |
|------|------|
| path_bias | **M 主**，跨部门多线时 H |
| p_mode_bias | P2/P4，要人话进度 |
| opener | 「这季度要推进哪件业务结果？成功时外面能看见什么？」 |
| example_chips | 业务结果一页计划 · 多部门本波目录 |
| clarify_depth | soft2 |
| artifact | single_plan / index_multi |
| primary_cta | 拆成步骤 / 生成本波 |
| direct_exec | hide |
| split_grain | balanced |
| parallel_appetite | task_parallel；H 时 plan hint |
| confirm_tone | normal |
| cost_posture | balanced |
| done_when_lexicon | 业务结果可观察 · 负责人能汇报 |
| non_goal_preset | 先不做：技术债大扫除当业务结果 |
| progress_metaphor | **管事** |
| 沟通一句 | 「像管项目：计划→分工→进度→齐了没。」 |

---

### 2.10 次受众开发者 · `dev`（不抢主路径）

| 旋钮 | 默认 |
|------|------|
| path_bias | M/H 随意；能力全开在 **高级** |
| 主路径 | 仍人话；provider/日志/TUI **折叠** |
| direct_exec | 高级可 offer |
| 沟通 | 不占用空态第一例子 |

---

## 3. 对照总表（一眼看「优化差在哪」）

| 画像 | path 主/副 | 开场域 | 澄清 | 主 CTA | 直接执行 | 并行食欲 | 确认 | 验收词核心 |
|------|------------|--------|------|--------|----------|----------|------|------------|
| founder | L / M | 客户可看 | none | 生成并看看 | offer | task | light | 演示 3 分钟 |
| pm | M / H | PRD/纪要 | soft2 | 拆成步骤/目录 | hide | task+plan | normal | 清单可勾 |
| designer | M / L | 第一眼/主钮 | soft1 | 拆成可预览 | hide | task 谨慎 | normal | 预览像意图 |
| ops | **H** / M | 本波几页截止 | soft2 | 生成本波目录 | hide | **plan+task** | normal | 页开表收语齐 |
| creator | L / M | 链接+语气 | none | 生成并看看 | offer | serial | light | 能发链接 |
| ecom | M / H | 卖点+禁编造 | soft2 | 拆成上架步骤 | danger | 多品分计划 | normal | **上架清单** |
| edu | M | 学员路径 | soft2 | 拆成开课步骤 | hide | serial | normal | 学员走通 |
| admin | M | 正式/必审 | soft2 | 草案等人确认 | **hide** | serial | **strict** | 存档未乱改 |
| biz_lead | M / H | 业务结果 | soft2 | 拆步/本波 | hide | task | normal | 可汇报 |

**结论**：九行在 **8+ 个旋钮** 上不同；不是三行 L/M/H。

---

## 4. 空态怎么用画像（仍 ≤3 概念）

**不要**：进门「你是电商还是 HR？」（问卷感、概念爆）。  

**要**（三选一信息架构）：

```text
A. 本次怎么干？ [快试 L] [一份计划 M] [多需求 H]   ← 形态
B. 主输入 + 占位随「场景芯片」变（芯片=弱画像，可跳过）
   芯片例：验证想法 | 写需求计划 | 大促多页 | 上架详情 | 开课说明 | 制度发文 …
C. 点芯片 → 套用 §2 旋钮（path_bias、opener、examples、cta…）
```

同屏概念仍 ≤3：**怎么干 · 输入 ·（可选）场景例子**。  
场景芯片是 **例子不是职业身份证**。

---

## 5. 与 L/M/H、P1–P6 的合成例

| 用户此刻 | 合成 |
|----------|------|
| 电商 + 只想改一版主图文案 | 芯片上架 + 用户选 L → ecom 验收词仍「上架清单」，但 coarse 短路径 |
| PM + 周末试一句 | 用户选 L → pm 的 direct 仍 hide，CTA「生成并看看」但强调保存再拆 |
| 运营 + 单页 | path 用 M，但 opener/进度仍「战役」减配，不是换成 founder 口吻 |
| 行政 + 想快 | 不可把 confirm 降 light；只缩澄清，**strict 保留** |

**硬规则**：画像可被用户选择覆盖 path；**不可**被覆盖的安全旋钮：admin 的 strict/hide direct；全局 confirm 红线。

---

## 6. 实施落点（W0 不必一次九全）

| 优先级 | 动作 |
|--------|------|
| P0 | 配置表或 JS/文案字典：`PersonaId → 旋钮`；空态芯片 6 个映射到画像 |
| P0 | opener / example_chips / primary_cta / direct_exec 四键先接上 |
| P1 | clarify_depth / done_when 提示 / non_goal_preset 进写作 prompt |
| P2 | parallel_appetite / split_grain 进拆分 grain_hint |
| P3 | 项目级记住上次芯片 |

验收：任意两画像并排，**用户能读出不同开场、不同主按钮、不同验收话**——不再是「都是快试/计划/多需求」三个壳。

---

## 7. 修订

| 日期 | 说明 |
|------|------|
| 2026-07-28 | 回应「只有 3 个路径没优化」：九画像 × 多维旋钮 |
