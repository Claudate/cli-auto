# 字体加载说明（可选）

真源：`docs/runtime-prompts/ui-typography-systems.md`（与色系同 kit 名）。

Kits 默认使用 **系统栈 + 常见 Web 字体名**，零构建即可预览。  
生产若需 Web 字体，在页面 `<head>` 自行加入开源字体（Google Fonts / 自托管 OFL），并保持 kit 中 `--font-display` / `--font-body` / `--font-ui` 变量名不变。

| kit | display | body / ui | 建议增强（可选 CDN / 自托管） |
|-----|---------|-----------|--------------------------------|
| western-saas | Plus Jakarta Sans 栈 | 系统 UI 栈 | Plus Jakarta Sans 或 Geist |
| nordic | Source Serif 4 | Source Sans 3 | Source Serif 4 + Source Sans 3 |
| cn-ink | Noto Serif SC / 宋体栈 | Noto Sans SC | 思源宋 + 思源黑；短标题可霞鹜文楷 |
| shanshui | LXGW WenKai / 楷体栈 | Noto Sans SC | 霞鹜文楷（**仅短标题**）+ Noto Sans SC |
| cn-festive | Noto Sans SC 粗展示 | Noto Sans SC | 站酷高端黑 / 马善政毛笔楷（OFL，仅节日标题） |
| jp-wa | Noto Serif JP | Noto Sans JP | Shippori Mincho + Noto Sans JP |
| jp-minimal | Noto Sans JP | 同 | Noto Sans JP 或 M PLUS 1p（字重拉开） |
| ios-hig | -apple-system / SF 栈 | 同 + PingFang | 少加载自定义；中文 PingFang / Noto Sans SC |
| material | Roboto | Roboto + Noto Sans SC | Roboto / Inter 回落 |
| fluent | Segoe UI 栈 | 同 + 雅黑/Noto | 系统即可 |
| ant-design | Ant 默认系统栈 | 同 | 系统 + Noto Sans SC |
| wechat-lite | PingFang SC 栈 | 同 | 系统中文栈，少英文艺术字 |
| startup-dark | 系统 UI 栈 | 同 | Inter；代码点缀可用 JetBrains Mono |
| fintech | Source Sans 3 | 同 | Source Sans 3 / Inter |
| edu-soft | Nunito / Plus Jakarta | 同 + Noto Sans SC | Nunito |

## 硬纪律（实现时）

1. **少字体**：≤2 个加载字体族（display + body/ui）。  
2. **书法/艺术字**只用于 Hero / 品牌 / 章节短标题（建议 ≤12 字中文 / ≤6 词英文）。  
3. **禁止**正文、按钮、表单、法律页脚用楷/隶/行书或 script。  
4. 每个角色保留系统回落；`font-display: swap`；中文优先子集。  
5. CSS 只引用变量：`h1/h2` → `--font-display`；`body` → `--font-body`；`button/nav/input` → `--font-ui`（见 `shared/base.css`）。

## demos 如何挂上

```html
<link rel="stylesheet" href="../../shared/base.css" />
<link rel="stylesheet" href="../../kits/<kit-id>.css" />
<link rel="stylesheet" href="../../shells/<shell>/shell.css" />
```

换 kit = 换第二行；组件类名与间距不变。
