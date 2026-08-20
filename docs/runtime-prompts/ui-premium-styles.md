# UI 高级风格配方（MotionSites 328 例提炼）

> 注入点：聊天 + 拆分 Agent 追加（用户写「高端 / 好看 / 高级感 / 质感」类网站时选用一套）。
> 来源：`public/MotionSites会员版` 328 条付费 prompt 统计提炼（2026-08-20）；配色闸门对齐 michael-design Curated Palette Library——只借手法，色值按品类映射到现有 color kits。

## 用法

一次只选 **一套** 配方贯穿全站；禁止把两套的 display 字体或背景策略混在一起。
未指定时默认「A 极简亮色」，用户明说暗色/奢侈/电影感才换 B/E。

## 配方 A · 极简亮色（默认 · SaaS/工具/AI）

- **底色**：白 / `zinc-50` 交替分节；前景 `zinc-950`；主 CTA `blue-600` 或 `emerald-600`，辅助 `indigo-600` 只用于链接与选中态。
- **字体**：display `Space Grotesk` / `Geist`（`tracking-tight`）+ body `Inter`（400/500/600）。
- **签名效果（选 1）**：大号网格背景（细线 `zinc-200`，中心径向遮罩渐隐）或 hero 单一软光斑（`blur-3xl opacity-30`，同色系，禁止彩虹）。
- **微交互**：卡片 hover `-translate-y-1` + shadow 加深；主按钮 hover `scale-[1.02]` active `scale-[0.97]`。
- 适合：任何 B 端工具、AI 产品、dashboard 落地页。

## 配方 B · 暗色奢华编辑风（agency / 奢侈 / 摄影作品集）

- **底色**：近黑 `stone-950` / `zinc-950`；前景 `stone-50`；表面 `stone-900`（**卡片必须比画布亮**）；点缀金色 `yellow-600/700` 或纯白反转 CTA。
- **字体**：display **衬线斜体** `Instrument Serif italic` + body `Barlow` / `Inter`。衬线斜体是这批素材里最高频的「高级感开关」（257/328 提到衬线）。
- **签名效果**：liquid glass——`backdrop-filter: blur(4-16px)` + 半透明白 `rgba(255,255,255,.01~.12)` 底 + inset 高光 `inset 0 1px 1px rgba(255,255,255,.1)` + ::before 渐变描边（上下亮、中间透明）。
- **禁区**：整屏霓虹、彩色渐变文字；发光只允许一处（hover 高光或 logo）。

## 配方 C · 温暖有机（民宿/咖啡/品牌/生活方式）

- **底色**：暖纸 `#FAF7F2`(stone-100 近似) / `#FFFBF5`；前景 `stone-800`；CTA `amber-700`；辅助 `green-800`。
- **字体**：display `Fraunces` / `Instrument Serif` + body `DM Sans`。
- **手法**：大幅真实照片（圆角 `rounded-2xl` 起）、Ken Burns 慢推（`scale 1→1.06, 15-20s`）、大字距小号 uppercase 标签。

## 配方 D · 粗犷海报风（活动/潮流/作品集）

- **底色**：单色实底（米白或近黑二选一）+ 一个高饱和强调色。
- **字体**：display `Anton` / `Archivo Black`（全大写、超大、`leading-none`）+ body `Inter Tight`。
- **手法**：超大排版即视觉（type-as-image）、marquee 无缝滚动条、描边字/双色字，其余全部留白。

## 配方 E · 电影感视频 hero（产品发布/agency）

- **底色**：同 B 或中性；全屏 `<video autoplay muted loop playsinline object-cover>` 压 z-0，内容 z-10。
- **叠层**：暗角渐变 `bg-gradient-to-b from-black/60 via-black/30` 保证文字 AA 对比。
- **降级**：无视频时用静帧图 + Ken Burns；移动端可换静帧省流量。

## 通用硬规则（来自 328 例的共性，任何配方都必须过）

1. **动效系统而非零散特效**：整页一条 easing `cubic-bezier(.16,1,.3,1)` + 三档时长（micro 0.12-0.2s / reveal 0.6-0.8s / ambient 12-20s）；每页 **1 个签名时刻**（parallax / sticky 叙事 / 横向媒体 rail / liquid glass，四选一）。
2. **分区 reveal**：每个区块 `whileInView` fade-up 24px 0.6-0.8s，子项 stagger 0.08-0.12s，`once: true`；移动端 24px→12px 并取消 parallax/pin；`prefers-reduced-motion` 全部降为 opacity ≤300ms。
3. **滚动进度条 / 圆点导航**：暗色长页二选一加上，即刻提升「作品感」。
4. **对比度**：暗色页正文 ≥ `stone-300`；玻璃容器内文字必须再压一层暗底，不许直接放在 blur 上。
5. **图标**：lucide 一套到底，线宽统一 1.5；禁止 emoji 混排。
6. **内容密度**：空壳三卡片=假网站；每个区块至少 6-8 条真实具体条目（价格/日期/评分带评价数）。

## 与现有配方的关系

- 布局/区块顺序：仍按 `ui-layout-systems.md`；本文件只管**视觉风格层**。
- 色板落地：写计划时引用 `ui-color-systems.md` 的 kit 名，不在此处发明新 hex。
- 动效白名单与实现：`ui-motion-effects.md`；本文件的参数与之合并使用。
