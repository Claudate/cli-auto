# runtime-prompts — 软件内底层提示（文档加载）

> **真源**：本目录 Markdown。运行时由 `domain/chat` 加载注入 **聊天写计划 / 拆分 Agent / 规划器**。  
> **不是** Claude Code skill；**不是**写死在 `.rs` 字符串里的长文。

## 文件

| 文件 | 注入点 |
|------|--------|
| [`chat-plan-writing.md`](./chat-plan-writing.md) | 桌面聊天 system prompt（计划写作助手） |
| [`ui-delivery-recipes.md`](./ui-delivery-recipes.md) | 聊天 + 拆分 **优先**（效果配方：布局·色·字·文案·动效·图·后端） |
| [`ui-layout-systems.md`](./ui-layout-systems.md) | 聊天 + 拆分 Agent 追加（站点类型 · 区块顺序 · 变体） |
| [`ui-color-systems.md`](./ui-color-systems.md) | 聊天 + 拆分 Agent 追加（色系 kit / CSS token） |
| [`ui-typography-systems.md`](./ui-typography-systems.md) | 聊天 + 拆分 Agent 追加（字体包 display/body/ui · 与色系同 kit） |
| [`ui-copy-systems.md`](./ui-copy-systems.md) | 聊天 + 拆分 Agent 追加（网站+App 界面文案 / 微文案） |
| [`ui-motion-effects.md`](./ui-motion-effects.md) | 聊天 + 拆分 Agent 追加（动效档 · 开源白名单 · reduced-motion） |
| [`backend-architecture.md`](./backend-architecture.md) | 聊天 + 拆分 Agent 追加（交付深度 A–D · 语言 · MVC/MVVM/DDD） |
| [`split-agent-delivery.md`](./split-agent-delivery.md) | Mode B 拆分 Agent system prompt 追加段 |
| [`planner-greenfield-stack.md`](./planner-greenfield-stack.md) | 旧路径 LLM Planner · greenfield 模式追加 |
| [`landing-gates.md`](./landing-gates.md) | **不注入 LLM**；给人与 `scripts/check-landing-gates.sh` 用 |

相关非注入资产：

| 路径 | 用途 |
|------|------|
| [`examples/marketing-landing-reference/SPEC.md`](../../examples/marketing-landing-reference/SPEC.md) | 营销站区块节奏 reference |
| [`scripts/check-landing-gates.sh`](../../scripts/check-landing-gates.sh) | 假域名 / 页脚主 CTA 等门禁 |

## 加载顺序（先命中先用）

1. 环境变量 `CCO_RUNTIME_PROMPTS_DIR`（目录，内含上表文件名）  
2. `~/.cco/runtime-prompts/`（用户覆盖，改完重启 cco 进程或下一次读盘）  
3. 从当前工作目录向上查找 `docs/runtime-prompts/`（开发仓）  
4. 可执行文件旁 `runtime-prompts/` 或 macOS `../Resources/runtime-prompts/`（打包资源，可选）  
5. **编译期嵌入**本目录文件（`include_str!`）——仅当磁盘皆无时回落，保证安装包可跑

## 怎么改

- **改产品默认**：直接改本目录 md → 提交；开发时 cwd 在仓内即可生效（若进程已 Once 缓存，重启 CLI/桌面）。  
- **本机覆盖**：`mkdir -p ~/.cco/runtime-prompts && cp docs/runtime-prompts/*.md ~/.cco/runtime-prompts/` 后编辑。  
- **CI/定制包**：设 `CCO_RUNTIME_PROMPTS_DIR=/path/to/dir`。

## 写作约束

- 中文为主；面向 PM/出海；避免把内部 ID 当主路径第一句。  
- 保持可注入：不要依赖仓库外绝对路径。  
- 变更后跑：`cargo test -p cco plan_writing_guidance`（或 domain chat 相关测）。

[PROTOCOL]: 增删文件名须同步 `src/domain/chat/plan_writing_guidance.rs` 常量与本表。
