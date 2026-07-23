## 色彩体系（写计划 + 做页面时必须选一套）

目标：站点**一眼有气质**，不是默认灰蓝或随机彩虹。  
适用：出海落地页、个人站、营销站、产品介绍页；brownfield 跟现有 token，不另起第二套。

### 怎么选（先决策再写 CSS）

| 用户/场景线索 | 默认 kit | 备选 |
|---------------|----------|------|
| 出海 SaaS / B2B / 英文营销 | **western-saas** | nordic |
| 硅谷 / Linear·Stripe 感 | **western-saas** | — |
| **iOS / Apple** 感 | **ios-hig** | nordic |
| **Google / Material / 安卓** | **material** | western-saas |
| **微软 Fluent** / 企业协作 | **fluent** | western-saas |
| **Ant / 中后台** | **ant-design** | material |
| **微信 / 小程序** 感 | **wechat-lite** | cn-ink |
| 深色科技 / 开发者 | **startup-dark** | western-saas |
| 金融可信 | **fintech** | western-saas |
| 教育柔和 | **edu-soft** | nordic |
| 中国品牌叙事 / 茶文旅 | **cn-ink** 或 **cn-festive** | shanshui |
| 国风 / 山水 | **shanshui** | cn-ink |
| 日系 | **jp-wa** | jp-minimal |
| 北欧冷静 | **nordic** | western-saas |
| 用户品牌色 | **custom** | — |

用户说「默认/你定」：优先 `ui-delivery-recipes` 配方行内 kit；否则按下表。  
用户点名「iOS/苹果/Material/谷歌/Fluent/安卓/微信/国风/山水/日系」：锁对应 kit，禁止混两套主色。  
平台 kit 为 **Web 气质近似**，非官方组件二进制；真原生另遵 HIG/Material。

### 硬纪律（所有 kit 共用）

1. **少色**：1 主色 + 中性阶（至少 4 级）+ 1 强调；禁止超过 2 个高饱和色抢 CTA。  
2. **角色绑定**：`--color-bg` / `--color-surface` / `--color-text` / `--color-muted` / `--color-border` / `--color-primary` / `--color-primary-fg` / `--color-accent` / `--color-cta-band` / `--color-footer`。  
3. **底 CTA ≠ Footer**：`--color-cta-band` 与 `--color-footer` 必须可分辨（不同底或分隔线+间距）；禁止两段同色深底粘连。  
4. **对比**：正文与底对比大致达标（浅底深字 / 深底浅字）；主 CTA 与底对比醒目。  
5. **实现**：用 CSS 变量（`:root` 或 `[data-theme="kit-id"]`）；**禁止**页面各处写死互不相关的 hex。  
6. **插图/摄影**：色温跟 kit（冷灰蓝 / 暖纸 / 墨青 / 和纸），禁止与 token 打架的霓虹滤镜。  
7. **不要**：紫粉渐变默认皮肤、五颜六色图标墙、用 emoji 当品牌色块。

### Kit 色板（可直接写进 CSS 变量）

下列 hex 为**可开工默认**；微调 ±5% 明度可以，换掉主色关系则算换 kit。

#### 1) `western-saas` — 欧美现代 SaaS

气质：冷静、可信、留白多；主 CTA 实心高对比。

| Token | Hex | 用途 |
|-------|-----|------|
| bg | `#F8FAFC` | 页底 |
| surface | `#FFFFFF` | 卡片 |
| text | `#0F172A` | 正文 |
| muted | `#64748B` | 次文 |
| border | `#E2E8F0` | 线 |
| primary | `#2563EB` | 主按钮/链 |
| primary-fg | `#FFFFFF` | 主按钮字 |
| accent | `#0EA5E9` | 点缀/次强调 |
| cta-band | `#0F172A` | 底转化带 |
| footer | `#F1F5F9` | 页脚（浅，与 cta-band 分层） |

#### 2) `nordic` — 北欧冷静

气质：雾灰、低饱和、少装饰。

| Token | Hex |
|-------|-----|
| bg | `#F4F1EC` |
| surface | `#FFFCFA` |
| text | `#1C1917` |
| muted | `#78716C` |
| border | `#E7E5E4` |
| primary | `#292524` |
| primary-fg | `#FAFAF9` |
| accent | `#0F766E` |
| cta-band | `#1C1917` |
| footer | `#E7E5E4` |

#### 3) `cn-ink` — 中国当代 / 墨色国风（偏产品站）

气质：墨黑 + 一点朱红或石青；不堆龙纹，靠留白与衬线/标题节奏。

| Token | Hex |
|-------|-----|
| bg | `#F7F4EF` |
| surface | `#FFFcf7` |
| text | `#1A1A1A` |
| muted | `#6B6560` |
| border | `#E4DDD4` |
| primary | `#8C1F28` |
| primary-fg | `#FFF8F5` |
| accent | `#2F5D50` |
| cta-band | `#1A1A1A` |
| footer | `#EFE8DF` |

#### 4) `cn-festive` — 中国喜庆 / 大促叙事（慎用日常产品）

气质：红金高识别；仍保持 1 主 + 1 强调，正文可读。

| Token | Hex |
|-------|-----|
| bg | `#FFF8F5` |
| surface | `#FFFFFF` |
| text | `#2B0B0E` |
| muted | `#8A5A5A` |
| border | `#F0D9D4` |
| primary | `#C8102E` |
| primary-fg | `#FFFFFF` |
| accent | `#C5A46E` |
| cta-band | `#8B0000` |
| footer | `#2B0B0E` |

Footer 深色时：页脚文字用浅色；底 CTA 带与 footer 用不同红阶或加分隔，禁止两段完全同色。

#### 5) `shanshui` — 山水 / 水墨意境

气质：远山青、雾、宣纸；适合文旅、内容、品牌故事；**少大红大紫**。

| Token | Hex |
|-------|-----|
| bg | `#F3F0E8` |
| surface | `#FBFAF6` |
| text | `#1E2A2F` |
| muted | `#5C6B73` |
| border | `#D9D3C5` |
| primary | `#3D5A5B` |
| primary-fg | `#F7F5EF` |
| accent | `#8B9A7D` |
| cta-band | `#2C3E40` |
| footer | `#E8E2D6` |

装饰：细线、淡远山 SVG、低对比水墨纹理即可；禁止满屏国潮贴纸。

#### 6) `jp-wa` — 和风（温暖纸感）

气质：和纸、朱、墨；适合日料/匠人/日系生活品牌。

| Token | Hex |
|-------|-----|
| bg | `#F7F3EB` |
| surface | `#FFFDF8` |
| text | `#2A2522` |
| muted | `#7A7168` |
| border | `#E5DCCF` |
| primary | `#B54A3C` |
| primary-fg | `#FFF8F5` |
| accent | `#4A6670` |
| cta-band | `#2A2522` |
| footer | `#EFE8DC` |

#### 7) `jp-minimal` — 日系极简（无印感）

气质：大量留白、近中性、弱品牌色。

| Token | Hex |
|-------|-----|
| bg | `#FAFAF8` |
| surface | `#FFFFFF` |
| text | `#222222` |
| muted | `#8A8A8A` |
| border | `#E8E8E4` |
| primary | `#222222` |
| primary-fg | `#FAFAF8` |
| accent | `#6B7C5C` |
| cta-band | `#111111` |
| footer | `#F0F0EC` |

#### 8) `ios-hig` — Apple / iOS 气质（Web 近似）

气质：大留白、大圆角感、系统蓝、细分割；CTA 像系统按钮。

| Token | Hex |
|-------|-----|
| bg | `#F2F2F7` |
| surface | `#FFFFFF` |
| text | `#1C1C1E` |
| muted | `#8E8E93` |
| border | `#C6C6C8` |
| primary | `#007AFF` |
| primary-fg | `#FFFFFF` |
| accent | `#5856D6` |
| cta-band | `#1C1C1E` |
| footer | `#E5E5EA` |

圆角建议：控件 ~10–12px；列表分组卡片感；图标 SF Symbols 风格线标（Web 用 Lucide 细线近似）。

#### 9) `material` — Google Material 气质

气质：主色块、容器色、中等圆角；强调可访问对比。

| Token | Hex |
|-------|-----|
| bg | `#FFFBFE` |
| surface | `#FFFFFF` |
| text | `#1C1B1F` |
| muted | `#49454F` |
| border | `#CAC4D0` |
| primary | `#6750A4` |
| primary-fg | `#FFFFFF` |
| accent | `#625B71` |
| cta-band | `#21005D` |
| footer | `#F3EDF7` |

FAB/主按钮可用 filled；次按钮 tonal/outline 语义用边框+主色字。

#### 10) `fluent` — Microsoft Fluent 气质

气质：中性灰底、品牌紫蓝、清晰分区；适合企业协作页。

| Token | Hex |
|-------|-----|
| bg | `#F5F5F5` |
| surface | `#FFFFFF` |
| text | `#242424` |
| muted | `#616161` |
| border | `#E0E0E0` |
| primary | `#5B5FC7` |
| primary-fg | `#FFFFFF` |
| accent | `#0078D4` |
| cta-band | `#292929` |
| footer | `#EBEBEB` |

#### 11) `ant-design` — Ant Design / 中后台气质

气质：浅蓝主色、密信息、表格友好；营销页慎用过密。

| Token | Hex |
|-------|-----|
| bg | `#F5F5F5` |
| surface | `#FFFFFF` |
| text | `#000000E0` 实现可用 `#262626` |
| muted | `#00000073` 实现可用 `#8C8C8C` |
| border | `#D9D9D9` |
| primary | `#1677FF` |
| primary-fg | `#FFFFFF` |
| accent | `#13C2C2` |
| cta-band | `#001529` |
| footer | `#F0F0F0` |

#### 12) `wechat-lite` — 微信/轻社交中国消费气质

气质：微信绿点缀、白底、亲和；适合小程序介绍/社群工具落地页。

| Token | Hex |
|-------|-----|
| bg | `#EDEDED` |
| surface | `#FFFFFF` |
| text | `#191919` |
| muted | `#888888` |
| border | `#E5E5E5` |
| primary | `#07C160` |
| primary-fg | `#FFFFFF` |
| accent | `#576B95` |
| cta-band | `#07C160` |
| footer | `#F7F7F7` |

底 CTA 用主绿时 footer 必须浅灰，禁止两段同绿粘连。

#### 13) `startup-dark` — 深色科技 / 开发者

气质：深底、亮主色、高对比代码感。

| Token | Hex |
|-------|-----|
| bg | `#0B0F19` |
| surface | `#121826` |
| text | `#E5E7EB` |
| muted | `#9CA3AF` |
| border | `#1F2937` |
| primary | `#3B82F6` |
| primary-fg | `#FFFFFF` |
| accent | `#22D3EE` |
| cta-band | `#1E3A5F` |
| footer | `#030712` |

#### 14) `fintech` — 金融可信

气质：深蓝绿、克制金点缀、稳重。

| Token | Hex |
|-------|-----|
| bg | `#F4F7F5` |
| surface | `#FFFFFF` |
| text | `#0F172A` |
| muted | `#64748B` |
| border | `#D1D9E0` |
| primary | `#0F766E` |
| primary-fg | `#FFFFFF` |
| accent | `#B45309` |
| cta-band | `#134E4A` |
| footer | `#E2E8F0` |

#### 15) `edu-soft` — 教育柔和

气质：浅紫/柔蓝、友好大按钮。

| Token | Hex |
|-------|-----|
| bg | `#F8F7FF` |
| surface | `#FFFFFF` |
| text | `#1E1B4B` |
| muted | `#6B7280` |
| border | `#E9E5FF` |
| primary | `#4F46E5` |
| primary-fg | `#FFFFFF` |
| accent | `#F59E0B` |
| cta-band | `#312E81` |
| footer | `#EEF2FF` |

#### 16) `custom` — 品牌指定

主色 1 + 中性阶 + 强调 1；角色 token 仍齐全。

### 落盘模板（实现时复制）

```css
:root, [data-theme="western-saas"] {
  --color-bg: #F8FAFC;
  --color-surface: #FFFFFF;
  --color-text: #0F172A;
  --color-muted: #64748B;
  --color-border: #E2E8F0;
  --color-primary: #2563EB;
  --color-primary-fg: #FFFFFF;
  --color-accent: #0EA5E9;
  --color-cta-band: #0F172A;
  --color-footer: #F1F5F9;
}
body { background: var(--color-bg); color: var(--color-text); }
.btn-primary { background: var(--color-primary); color: var(--color-primary-fg); }
.band-cta { background: var(--color-cta-band); }
.site-footer { background: var(--color-footer); }
```

换 kit：改 `data-theme` 或换一组变量，**不要**在组件里散落新 hex。

### 写进计划 / 步骤时

- **计划**：在「建议技术」或单独「视觉」写：`色系 kit = xxx` + 气质一句；成功标准含「主 CTA 与底对比清晰」「底 CTA 与页脚分层」「`:root` 变量齐全」。  
- **拆分 do 步骤**：body【怎样算做完】含 token 落点文件（如 `styles/tokens.css` / `global.css`）；禁止只写「好看一点」。  
- **结构**仍对齐 `examples/marketing-landing-reference/SPEC.md`；**色跟 kit，节奏跟 SPEC**。
