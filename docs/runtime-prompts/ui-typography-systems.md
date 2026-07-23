## 字体体系（写计划 + 做页面时与色系 kit 配套）

目标：站点/App **气质靠字体一半**——山水站不是系统默认黑体堆满屏；出海 SaaS 也不是乱堆艺术花体。  
**与色系同 kit 名**（`western-saas` / `shanshui` / …）：先定色系 kit，字体默认跟同一 kit。  
整包场景默认见 `ui-delivery-recipes.md`（如 R-shanshui → 文楷标题 + 黑体 UI）。

### 角色三分（全站最多 2～3 个字体族）

| 角色 | CSS 变量 | 用途 | 禁忌 |
|------|----------|------|------|
| **展示** `display` | `--font-display` | Hero H1、品牌名、章节大标题、海报句 | **禁止**长正文、小字表单、密排导航 |
| **正文** `body` | `--font-body` | 段落、列表、说明 | 花体/隶书大段连用 |
| **界面** `ui` | `--font-ui` | 按钮、导航、表格、输入框 | 手写体/书法体 |

默认：`ui` 可与 `body` 同族以减请求；`display` 才是「艺术感」入口。

### 硬纪律

1. **少字体**：≤2 个加载字体族（display + body/ui）；第三族仅当品牌强制。  
2. **可读优先**：正文必须清晰；书法/艺术字 **只用于标题/短句**（建议 ≤12 字中文 / ≤6 词英文）。  
3. **中文书法体**（隶、楷、行、魏碑风）：✅ Hero/品牌/章节标题；❌ 长文、按钮、表单、法律页脚。  
4. **西文艺术体 / 衬线展示**：✅ 大标题与引用；❌ 全站 body 用极细 script。  
5. **回落栈**：每个角色写系统回落（见下表）；断网/失败仍可读。  
6. **许可**：优先 **SIL OFL / Apache / MIT** 开源或系统自带；**禁止**未授权商用字体、禁止只写「用隶书」却不给可下载族名。  
7. **实现**：`@font-face` 或 Google Fonts / 字由等合法 CDN + CSS 变量；`font-display: swap`；中文子集或按需加载，避免一次拉全字库拖垮首屏。  
8. **排版**：中文 `text-wrap: balance` / 人工断句；行高 body 约 1.6～1.8；字距 display 可略收、body 勿乱 tracking。  
9. **App/桌面**：移动端优先系统栈 + 少量 display；原生可用平台字体（San Francisco / 苹方 / Noto），展示字仍限短标题。  
10. **brownfield**：跟现有 token；不另起冲突第二套字体。

### 与色系 kit 对齐的默认字体包

下列为**可开工默认**（Web 友好 · 开源优先）。用户点名「必须方正/汉仪某某」→ 记入计划并核许可，否则用表内替代。

#### 1) `western-saas` — 欧美现代 SaaS

| 角色 | 推荐族 | 气质 |
|------|--------|------|
| display | **Inter** 或 **Geist** / **Satoshi**（有授权时） | 几何无衬线、产品感 |
| body/ui | **Inter** 或 **System UI** 栈 | 高密度 UI 清晰 |

备选 display：**Plus Jakarta Sans**、**DM Sans**。  
禁止：Comic Sans、默认 Times 当品牌、正文用 script。

系统回落：
`ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif`

#### 2) `nordic` — 北欧冷静

| 角色 | 推荐族 |
|------|--------|
| display | **Source Serif 4** 或 **Literata**（克制衬线） |
| body/ui | **Source Sans 3** 或 **IBM Plex Sans** |

气质：纸感、低装饰；勿粗黑大字报。

#### 3) `cn-ink` — 中国当代 / 墨色

| 角色 | 推荐族 | 说明 |
|------|--------|------|
| display | **站酷庆科黄油体** / **霞鹜文楷**（LXGW WenKai）作标题 | 有气质但别整页楷书 |
| 或 display | **思源宋体**（Source Han Serif / Noto Serif SC）标题 | 更「出版/品牌」 |
| body | **思源黑体**（Noto Sans SC）或 **HarmonyOS Sans SC** 若可用 | 正文必黑体/无衬线清晰款 |
| ui | 同 body | 按钮导航可读 |

**楷/隶**：仅 H1/品牌短句；正文用黑体。

#### 4) `cn-festive` — 喜庆 / 大促

| 角色 | 推荐族 |
|------|--------|
| display | **站酷高端黑** 或粗黑展示 + 短句；可配 **马善政毛笔楷** 类展示（OFL）作节日标题 |
| body/ui | **Noto Sans SC** |

标题可稍张扬；表单/价格数字用等宽或 UI 黑体，避免毛笔价签难认。

#### 5) `shanshui` — 山水 / 水墨（你举的例子）

气质：远山、宣纸、诗意——**展示用书法感，正文仍要能读**。

| 角色 | 推荐 | 为何 |
|------|------|------|
| **display** | **霞鹜文楷**（LXGW WenKai）或 **霞鹜铭心宋**；需要更「碑/隶」感可用 **得意黑** 不作正文、或 **Source Han Serif SC** 大标题 | 楷书感适合诗意标题；**纯隶书**（如部分开源隶）只适合 2～8 字标题 |
| display 备选 | 系统 **楷体** 栈：`"Kaiti SC", "STKaiti", "KaiTi", serif`（本地演示快；Web 需自托管开源楷） | 演示档 A 可用 |
| **body** | **Noto Serif SC** 轻量 或 **Noto Sans SC** | 长文：宋体叙事 / 黑体说明二选一，**不要**全文隶书 |
| **ui** | **Noto Sans SC** | 导航、按钮必须清晰 |

**山水站不要**：全站华文隶书、正文行书、按钮用毛笔体。  
**要**：Hero 一句诗意标题用楷/文楷；能力卡片标题可用宋；其余黑体。

#### 6) `jp-wa` — 和风

| 角色 | 推荐族 |
|------|--------|
| display | **Shippori Mincho** / **Noto Serif JP** |
| body/ui | **Noto Sans JP** 或 **Zen Kaku Gothic New** |

短标题可用明朝；UI 用ゴシック。

#### 7) `jp-minimal` — 日系极简

| 角色 | 推荐族 |
|------|--------|
| display + body | **Noto Sans JP** 或 **M PLUS 1p**（字重拉开即可） |
| 强调 | 极大字重差 + 留白，少第二字体 |

#### 8) `ios-hig`

| 角色 | 推荐族 |
|------|--------|
| display/body/ui | **-apple-system / SF Pro 栈**（Web：`system-ui, -apple-system, "SF Pro Text", "Helvetica Neue", sans-serif`） |
| 中文 | **PingFang SC** / 苹方回落 + Noto Sans SC |

少加载自定义花体；靠字重与字号层级。

#### 9) `material`

| 角色 | 推荐族 |
|------|--------|
| display/body/ui | **Roboto** 或 **Google Sans**（有授权时）/ **Inter** 回落 |
| 中文 | **Noto Sans SC** |

#### 10) `fluent`

| 角色 | 推荐族 |
|------|--------|
| 全角色 | **Segoe UI** 栈：`"Segoe UI", system-ui, sans-serif`；中文 **微软雅黑** / Noto Sans SC |

#### 11) `ant-design`

| 角色 | 推荐族 |
|------|--------|
| 全角色 | **-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, "Noto Sans SC", sans-serif**（Ant 默认栈气质） |

#### 12) `wechat-lite`

| 角色 | 推荐族 |
|------|--------|
| 全角色 | **PingFang SC** / 系统中文栈；少用英文艺术字 |

#### 13) `startup-dark` / `fintech` / `edu-soft`

| kit | display | body/ui |
|-----|---------|---------|
| startup-dark | Inter / JetBrains Mono 仅代码 | Inter |
| fintech | Source Sans 3 / Inter | 同 |
| edu-soft | Nunito / Plus Jakarta（圆润） | 同 + Noto Sans SC |

#### 14) `custom` / 品牌指定

主展示 1 族 + 正文 1 族；写进计划并注明许可来源。

### 西文「艺术字」何时用

| 场景 | 可用 | 避免 |
|------|------|------|
| 时尚/餐饮/作品集 Hero | **Playfair Display**、**Cormorant**、**Libre Baskerville** | 正文全衬线细体 |
| 手作品牌短标 | **Caveat** / **Patrick Hand** 仅 Logo 级 | 导航、表单 |
| 开发者/SaaS | 保持 Inter 系，用字重/尺寸做层级 | 花体「显贵」 |

### 落盘模板（与色系变量并列）

```css
:root, [data-theme="shanshui"] {
  --font-display: "LXGW WenKai", "Kaiti SC", "STKaiti", "Noto Serif SC", serif;
  --font-body: "Noto Serif SC", "Source Han Serif SC", "Songti SC", serif;
  --font-ui: "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif;
}
[data-theme="western-saas"] {
  --font-display: "Inter", ui-sans-serif, system-ui, sans-serif;
  --font-body: "Inter", ui-sans-serif, system-ui, sans-serif;
  --font-ui: "Inter", ui-sans-serif, system-ui, sans-serif;
}
h1, h2, .brand-title { font-family: var(--font-display); }
body { font-family: var(--font-body); }
button, input, nav { font-family: var(--font-ui); }
```

加载示例（按需，勿全抄）：Google Fonts / 自托管 `fonts/` + `@font-face`；中文优先子集或文楷/思源的 web 子集包。

### 写进计划 / 拆分

- **计划「建议技术」或「视觉」**：`色系 kit = X` + `字体包 = display/body/ui 族名` + 一句气质（例：山水 · 标题文楷 · 正文宋/黑）。  
- **成功标准**：display 仅用于标题级；正文可读；CSS 变量齐全；回落栈存在；无未授权字体。  
- **拆分 do**：【改哪里】含 `tokens.css` / `fonts` 路径；【怎样算做完】含「Hero 用 display、按钮用 ui、长文不用书法体」。  
- **A 演示档**：可用系统楷体/苹方快速出效果；上线前换成可分发开源族。
