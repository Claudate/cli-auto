## 前端动效 / 特效（写计划 + 做页面时必选档位）

目标：页面**有质感**，但不挡主 CTA、不拖垮首屏。  
与色系 **同 kit 气质**；库名只从下方**白名单**选，同屏特效概念 **≤ 2**。  
整站默认档位优先抄 `ui-delivery-recipes`（出海 light、工具 none/light、山水 light 无粒子）。

### 1. 动效档（先选一档，写进「建议技术」）

| 档位 | 何时 | 默认做法 | 依赖上限 |
|------|------|----------|----------|
| **none** | 极简工具、表单后台、用户要无动画 | 仅必要 transition（focus/hover 可极短） | 0 动画库 |
| **light** | 演示 A、多数落地页默认 | CSS + 可选 IntersectionObserver 入场 | 0～1 库 |
| **brand** | 出海营销、品牌叙事、要「高级感」 | CSS + **一种**时间轴库（GSAP **或** anime 二选一）± 轻滚动 | 1～2 库 |
| **3d-hero** | 用户明确要 3D/WebGL 展示 | 仅 **一节** Hero；其余 light | + Three 或 OGL 1 个 |

用户说「默认/你定」：营销/出海站 → **light**（可升 brand 若强调氛围）；工具型 Web → **none/light**；山水国风 → **light**（SVG/雾，不默认粒子）。  
用户说「特效炫一点 / 滚动叙事」→ **brand**；说「3D/粒子宇宙」→ 确认性能后 **3d-hero**，并写移动端降级。

### 2. 硬纪律（所有档）

1. **主 CTA 第一**：动画不得遮挡、抖动或延迟可点主按钮。  
2. **同屏概念 ≤ 2**（例：区块滚入 + 按钮 hover；不要再叠粒子+打字机+3D）。  
3. **`prefers-reduced-motion: reduce`**：必须减弱或关闭非必要动画（CSS 媒体查询或库内 pause）。  
4. **移动端降级**：粒子减半/关；3D 改为静图或短视频封面；smooth scroll 可关。  
5. **LCP/性能**：首屏少大 JS；Lottie/Three 懒加载；禁止未压缩全屏循环抢主线程。  
6. **许可**：优先 MIT/Apache；GSAP **核心**可商用，**收费插件默认不用**。  
7. **插图门禁仍有效**：特效层里禁止 placehold 灰图。  
8. **中文**：少用逐字拆字长文；标题短 stagger 可以。

### 3. 开源白名单（只荐这些名；实现时写清用途一句）

| 用途 | 首选 | 备选 | 勿默认 |
|------|------|------|--------|
| 微交互 / 入场 | **CSS** / WAAPI | 自写 IntersectionObserver | 为 fade 拉 GSAP 全家 |
| 时间轴 / 编排 | **anime.js**（轻）或 **GSAP**（重叙事） | Motion（仅 React 栈） | 同时上 anime+GSAP |
| 滚动叙事 | **GSAP ScrollTrigger** | **Lenis** 丝滑滚动 | 全站改原生滚动无降级 |
| 简单滚入 | CSS 或 **AOS** | ScrollReveal | 与 ScrollTrigger 叠两套 |
| 粒子背景 | **tsparticles** 轻配置 | — | 全站 + 高数量；旧 particles.js 新项目慎用 |
| 3D 背景 | **Three.js** 或 **OGL** 仅 Hero | Vanta（重） | 多页每页 3D |
| 矢量动效 | **Lottie**（小环） | **Vivus** SVG 描边 | 巨大 JSON 首屏同步 |
| 文字强调 | CSS / **rough-notation** | Typed.js 一句 slogan | 整页打字机 |
| 轮播 | **Embla** 或 **Swiper** | Splide | 无证据区也上轮播 |
| 灯箱 | GLightbox / PhotoSwipe | — | — |
| 多页过渡 | View Transitions API | Swup/Barba | 单页落地硬上 PJAX |

### 4. 与色系 kit 搭配（气质）

| kit | 建议 | 少用 |
|-----|------|------|
| western-saas | light→brand：CSS 入场 + 按钮 hover；可选 GSAP/anime 一处 | 满屏粒子、花体 Typed |
| nordic | light：慢 fade、大留白 | Vanta/重 3D |
| shanshui / cn-ink | light：Vivus/SVG 线、雾 CSS、轻视差 | 科技粒子网、霓虹闪 |
| cn-festive | light+短时强调；可选轻粒子 | 全年吵闹循环 |
| jp-wa / jp-minimal | none/light：短 easing | 强视差、巨大自定义光标 |
| 演示 A | **light 或 none**；0～1 库 | Three 全家桶 |

### 5. 默认组合（可直接写进计划）

```text
动效档：light | brand | none | 3d-hero
库：无 | anime.js | gsap | gsap+lenis | tsparticles(轻) | lottie | embla | three(仅hero)
同屏：滚入 + 按钮微交互（示例）
降级：prefers-reduced-motion；移动端关粒子/3D
```

| 场景 | 默认一行 |
|------|----------|
| 出海落地页 | `light` · CSS 入场 + hover · 可选 anime 或 AOS |
| 品牌长叙事 | `brand` · GSAP + ScrollTrigger · 可选 Lenis |
| 作品集证据墙 | `light` + Embla/Swiper |
| 山水/文旅 | `light` · SVG/Vivus · 无粒子默认 |
| 明确 3D 产品 | `3d-hero` · Three/OGL 一节 + 静态回落图 |

### 6. 写进计划 / 拆分

- **建议技术**：`动效档 = …` + 库名（≤2）+ 一句话用途。  
- **成功标准**：主 CTA 可点不被挡；`prefers-reduced-motion` 可关动画；移动端可接受（无严重掉帧）；未引入白名单外堆砌库。  
- **拆分 do**：【改哪里】含 `css`/`js` 动效文件；【怎样算做完】写清档位与降级；禁止只写「加特效」。  
- **A 演示**：优先纯 CSS；非必要不上 3D/粒子。
