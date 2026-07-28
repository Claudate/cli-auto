# 营销 / 产品落地页 · 结构参考（Reference）

> **用途**：实现时对齐**节奏与层次**，不是抄品牌皮。  
> **真源关系**：纪律见 `docs/runtime-prompts/`；整包默认抄 **`ui-delivery-recipes` → R-overseas**；**本文件 = 站点类型 `marketing` 区块 reference**。其它类型见 `ui-layout-systems.md`。  
> **可运行底板**：[`examples/site-floor/shells/marketing`](../site-floor/shells/marketing/) + kit；配方组合见 [`site-floor/RECIPE-MAP.md`](../site-floor/RECIPE-MAP.md) · demo [`site-floor/demos/r-overseas`](../site-floor/demos/r-overseas/)。  
> **非**：本 SPEC 自身不是完整站点源码；实现应对齐本结构 **并**从 site-floor 起步，而非另发明五段同色深底。

## 一句话

给【谁】在【场景】得到【可观察结果】；主 CTA 动词唯一。  
界面文案细则（H1/CTA/空错态 + App 微文案）：`docs/runtime-prompts/ui-copy-systems.md`。

## 推荐区块顺序（不可乱序堆砌）

| 序 | 区块 | 必须 | 内容要点 | 禁止 |
|----|------|------|----------|------|
| 1 | **顶栏** | ✓ | 唯一产品主名 + 导航 ≤5 + 一个主 CTA | 公司名与产品名抢 Logo |
| 2 | **Hero** | ✓ | 结果导向 H1 + 导语 + **主 CTA + 次 CTA** +（建议）产品示意位 | 只有口号、无证据位 |
| 3 | **证据** | 建议 | 截图 / 对话样例 / 数据 / Logo 墙 择一 | 直接跳功能三卡片 |
| 4 | **能力** | ✓ | ≤3 张；每张 = 用户动作 → 可见结果 | 说明书目录腔 |
| 5 | **场景** | 可选 | ≤2 条受众路径；导语可断行，忌单字落行 | 长段居中难读 |
| 6 | **底 CTA 带** | ✓ | 唯一强转化带；可深色；**与页脚必须分层** | 与 footer 同色无边界 |
| 7 | **Footer** | ✓ | 链接 / 法务 / 次要入口 | **默认无主 CTA 大按钮** |

## 视觉纪律（下限）

1. 少色：1 主色 + 中性阶 + 1 强调；**先选 kit 再写页面**（`docs/runtime-prompts/ui-color-systems.md`：western-saas / nordic / cn-ink / cn-festive / shanshui / jp-wa / jp-minimal）。  
2. 底 CTA ≠ Footer 底色（或明确分隔线 + 间距）；用 `--color-cta-band` vs `--color-footer`。  
3. 中文标题：`text-wrap: balance` / 人工断句。  
4. 图标：开源线标（Lucide 等），禁止 emoji 按钮。  
5. 主 CTA 文案全站一致；出现位置建议：顶栏 + Hero + 底带（最多三处强按钮）。  
6. 颜色只进 CSS 变量；禁止组件内随机 hex 拼盘。  
7. 字体：与色系同 kit；`--font-display` / `--font-body` / `--font-ui`（见 `ui-typography-systems.md`）；书法/艺术字仅短标题，不作正文与按钮。  
8. 动效：档位 none/light/brand/3d-hero（见 `ui-motion-effects.md`）；库白名单且 ≤2；主 CTA 不被挡；`prefers-reduced-motion` 可关。  
9. **防死板**：区块职责按上表；Hero 构图/证据形态/能力区形态可做受控变体（见 `ui-layout-systems.md` §2.1），勿每站同一左文右图+三等分图标卡。

## 真实资产门禁（写进成功标准）

- [ ] 无 example.com 可见链（或计划注明仅演示）  
- [ ] 主 CTA URL 真实或本地 `#demo`  
- [ ] 顶栏唯一主名  
- [ ] 插图非占位图：图库 / 生成图 / 品牌素材；无 placehold·dummyimage·`placeholder.*`  
- [ ] `scripts/check-landing-gates.sh` 无 FAIL（含 G7 图片）  

## 人 30 秒走查

1. 3 秒内知给谁、干什么？  
2. 主按钮是否唯一醒目、文案可信？  
3. 点主按钮会去哪（真链 / 说明）？  
4. 滚到底：销售带与页脚是否分得清？  
5. 窄屏：导航与主 CTA 可达？  

## 实现落点提示（绿野）

- 静态 / Astro / 既有 SSG；部署 Pages/Vercel 类。  
- 文案与结构先于配色动画。  
- 对照本 SPEC 自检后再交门禁脚本。
