# LX2 · PendingUserGate 人话待办对象（optional 必停升级 · PM 可见）
## 目标
- 给谁：PM / 出海 / 非开发主受众。
- 场景：optional 任务「必停等确认」现在只是个布尔勾选（web `planNeedsOptionalConfirm`），PM 看不清「到底在等我回答什么」。
- 可观察结果：停住时拆分台/Run 台显示一句人话——「当前等你回答：是否执行可选任务 X」+ 一句 why。
## 范围
### 做
- `domain/run` 新增 `PendingUserGate{ kind, question, why }` 纯构造（复用 `status_line.rs` 的 `StatusOneLiner` 人话风格）。
- `app/run` 填充进 Run/Result DTO。
- web 只渲染该对象（rule 22 · 无 UI 业务策略）。
### 不做
- 不改「停/不停」的判定逻辑（仍是 optional 必停，符合规则 14）。
- 不引入 auto-start / 不绕过 confirm（规则 10/14）。
- 不做 InspectReview 等其它 GateKind 的全套（先 OptionalConfirm 一种，其余留枚举位）。
## 会失去什么
- 多一个 DTO 字段与渲染分支；换来 PM 停住时的明确待办感。风险低。
## 建议技术
- 交付深度：D 改现有（brownfield）
- 形态：全栈（Rust domain/app + web 渲染层）
- 语言/框架：Rust + 既有 web（冻结栈）
- 架构：domain 纯构造 → app 填 DTO → web 只渲染（rule 22）
- 界面文案：语气克制人话；主句「当前等你回答：X」；why 一句「该任务标了可选，需你确认」；无内部 ID/枚举名作第一句
- 为什么：把布尔 gate 升级成结构化人话对象，PM 心智清楚且不违反 confirm 唯一开跑。
## 任务大纲（V1）
1. `domain/run`：定义 `GateKind`（先 `OptionalConfirm`）+ `PendingUserGate` 纯构造函数；参照 `status_line.rs` 人话映射。
2. `app/run`：在现有 optional 必停判定处填充 `PendingUserGate` 进 Run/Result DTO（复用当前 `planNeedsOptionalConfirm` 对应的服务端判定点）。
3. web：`jobPoll.js` 停住分支从布尔渲染升级为渲染 `question`/`why`；无业务策略，只显示。
4. fake 五步桌面冒烟：optional 任务停住时目视「等你回答：X」正确出字。
## 成功标准（怎样算做完）
- [ ] 停住时拆分台/Run 台显示人话「当前等你回答：X」+ why 一句，无内部枚举/ID 作第一句
- [ ] domain 构造有单测；`app/run` 正确填 DTO；web 仅渲染（rule 22，无策略）
- [ ] 仍是「停住等 confirm」，无静默 auto-start（规则 10/14）
- [ ] 不改 schema/IPC 名（DTO 增字段向后兼容）；`cargo test` 绿
## 风险 / 待确认
- 假设：现有服务端 optional 判定点可直接挂 DTO 填充（需在 app/run 定位与 `planNeedsOptionalConfirm` 对应的落点）。
- DTO 增字段须向后兼容，旧渲染路径不炸。
## V1 边界
仅 `OptionalConfirm` 一种 GateKind 的构造 + 填充 + 渲染；其余 kind 留枚举位不实现。
