## 交付效果配方（把布局·色·字·动效·图·后端一次定齐）

目标：写计划/做站时**少纠结**，默认组合已经「好看 + 可验收」；细则仍以各专项 md 为准。  
**决策顺序（固定）**：

```text
交付深度 A–D → 要不要后端 → 站点类型 → 版式变体 2～4
→ 色系 kit（= 字体 kit）→ 界面文案（主 CTA/空错载）→ 动效档 → 图片 → 成功标准
```

用户说「默认/你定」：按下表对应行整包采用，计划里写配方 id + 一句假设。

---

### 1. 场景配方一览（直接抄）

#### 1.1 场景 / 行业

| 配方 id | 用户要什么 | 深度 | 站点类型 | 色系 kit | 动效 | 后端 |
|---------|------------|------|----------|----------|------|------|
| **R-overseas** | 出海落地页 / SaaS 介绍 | A 或 B | marketing | western-saas | light→brand | 无 / 表单 |
| **R-cn-brand** | 国风品牌 / 茶文旅 | A | story/marketing | cn-ink 或 shanshui | light | 无或表单 |
| **R-shanshui** | 山水 / 诗意叙事 | A | story | shanshui | light | 无 |
| **R-jp** | 日系生活 / 匠人 | A | marketing/story | jp-wa / jp-minimal | none/light | 无 |
| **R-portfolio** | 作品集 / 个人站 | A | portfolio | nordic/custom | light | 无或表单 |
| **R-waitlist** | 预热留资 | A/B | waitlist | western-saas/品牌 | none/light | 表单 |
| **R-docs** | 文档 / 博客 | A/B | content | nordic | none/light | 无或 SSG |
| **R-shop** | 小店 / 商品 | B | ecommerce | custom/cn-festive | light | 支付按需 |
| **R-tool** | 登录后 Web 工具 | B/C | app-shell | western-saas/品牌 | none/light | **API** |
| **R-admin** | 后台 / 控制台 | B/C | dashboard | ant-design 或 nordic | none | **API** |
| **R-event** | 活动报名 | A/B | event | cn-festive/brand | light | 表单/票务 |
| **R-fintech** | 金融/理财介绍 | A/B | marketing | fintech | light | 合规表单 |
| **R-edu** | 教育/课程落地 | A/B | marketing | edu-soft | light | 表单/B 登录 |
| **R-devtool** | 开发者工具 / 深色品牌 | A/B | marketing/app-shell | startup-dark | light/brand | 无或 API |

#### 1.2 平台 / 设计系统气质（用户说「像 iOS / 谷歌 / 微软…」）

| 配方 id | 用户话术 | 站点类型默认 | 色系 kit | 字/控件口音 | 动效 | 备注 |
|---------|----------|--------------|----------|-------------|------|------|
| **R-ios** | iOS、苹果风、Apple 感、Health 类 | marketing 或 app-shell | **ios-hig** | 系统栈/苹方；大圆角；细线图标 | light（spring 短） | Web 近似 HIG，非官方 UIKit |
| **R-material** | 谷歌、Material、Android 风 | marketing 或 app-shell | **material** | Roboto/Noto；filled 主钮 | light | 可对齐 M3 容器色思路 |
| **R-fluent** | 微软、Fluent、Teams/Office 感 | marketing 或 dashboard | **fluent** | Segoe；分区清晰 | none/light | 企业协作落地页 |
| **R-ant** | Ant Design、中后台、阿里风表格 | dashboard / app-shell | **ant-design** | Ant 系统栈 | none | 密信息；少营销大 Hero |
| **R-wechat** | 微信风、小程序介绍、社群工具 | marketing / waitlist | **wechat-lite** | 系统中文；绿 CTA | none/light | 底带与 footer 分层 |
| **R-android-app** | 安卓 App 壳 / 工具 | app-shell | material | 同 R-material | none/light | 导航底栏或侧栏 |

**组合规则**：平台配方与场景配方冲突时——**用户点名平台优先**（例：「出海但要 iOS 感」→ R-ios + 站点 marketing + 文案出海）。  
**禁止串味**：R-admin 用营销五段；R-shanshui 上科技粒子；R-ios 用微信绿主 CTA；R-material 全文书法。

---

### 2. 配方展开（实现时按此填满）

#### R-overseas（默认出海）

| 项 | 默认 |
|----|------|
| 版式变体 | Hero=A 或 B · 能力=三卡或 1+2 · 密度=中 · 装饰=细线 |
| 字体 | Inter / system（display=body=ui 可同族） |
| 动效 | CSS 入场 + 按钮 hover；可选 anime **或** AOS 其一 |
| 图片 | Hero：产品 UI 实图或生成界面图；证据：截图/数据图；能力：线标图标非 emoji |
| 后端 | 纯介绍 → 无；有「预约/注册」→ 第三方表单，勿首日自建用户系统 |
| 验收 | SPEC 节奏 + 门禁无 FAIL + 底 CTA≠footer |

#### R-shanshui / R-cn-brand

| 项 | 默认 |
|----|------|
| 版式变体 | Hero=D 全幅雾感 · 章节左右交替 · 密度=疏 · 装饰=雾/细线 |
| 字体 | display=文楷/思源宋短标题；body/ui=Noto Sans 或宋（长文） |
| 动效 | light：SVG 描边/淡入；**禁止**默认 tsparticles |
| 图片 | 山水/纸感/实景图库或生成图；色温跟 kit；Hero 图上字须遮罩保证对比 |
| 后端 | 默认无；咨询表单可边缘函数 |
| 验收 | 标题非全文隶书；主 CTA（若有）可读可点 |

#### R-portfolio

| 项 | 默认 |
|----|------|
| 版式变体 | 首屏作品栅格 · 错落 1+2 可选 · 密度=疏 |
| 字体 | 克制无衬线；display 仅姓名 |
| 动效 | light hover 即可 |
| 图片 | **每格真实作品图**（无占位）；灯箱可选 Embla/PhotoSwipe |
| 后端 | 联系表单可选 |
| 验收 | 首屏见作品，无两屏空口号 |

#### R-tool / R-admin

| 项 | 默认 |
|----|------|
| 版式变体 | 侧栏或顶栏壳 · 主操作右上 · 密度=中/密 |
| 字体 | UI 一栈（Inter/Noto Sans） |
| 动效 | **none/light**（仅 transition） |
| 图片 | 少插图；空态用线标+人话，**不用**大营销图 |
| 后端 | B：单体 MVC/轻分层 + 一库；语言用户点名优先，否则 Node 或 Go |
| 验收 | 登录/主 CRUD 真通；空/错/载有人话；无 DDD 空壳（除非 C） |

#### R-waitlist

| 项 | 默认 |
|----|------|
| 版式 | **一屏**：价值句 + 表单 + 主 CTA |
| 图 | 一张品牌/产品氛围图即可，可背景 |
| 后端 | 表单服务 / 轻 Worker |
| 验收 | 提交有成功态；无 example.com |

#### R-ios

| 项 | 默认 |
|----|------|
| 版式变体 | Hero=C 或 E · 密度=疏 · 大留白 · 卡片分组列表感 |
| 色/字 | kit=`ios-hig`；系统字体栈 |
| 动效 | light：短 spring/ease；页面切换勿花；reduced-motion |
| 图片 | 设备框内 UI 截图/生成；圆角与阴影克制 |
| 后端 | 展示 A；账号能力走 B |
| 验收 | 主 CTA 像系统按钮可读；非山寨苹果 Logo |

#### R-material

| 项 | 默认 |
|----|------|
| 版式 | Hero=A · 能力三卡或 tonal 表面分区 · 密度=中 |
| 色/字 | kit=`material`；Roboto/Noto |
| 动效 | light：标准 ease；可选容器色过渡 |
| 图片 | 产品 UI + 简洁插画；图标 Material/Lucide 线标 |
| 验收 | 主色对比足够；勿抄 Google 商标图形 |

#### R-fluent / R-ant / R-wechat

| 配方 | 要点 |
|------|------|
| R-fluent | 企业稳；顶栏+内容区；kit=fluent；动效 none/light |
| R-ant | **dashboard** 优先；表/筛/主操作；kit=ant-design；无销售五段 |
| R-wechat | 绿主 CTA；一屏说清；kit=wechat-lite；footer 勿同绿 |

#### R-fintech / R-edu / R-devtool

| 配方 | kit | 注意 |
|------|-----|------|
| R-fintech | fintech | 文案合规克制；证据用真数据口径 |
| R-edu | edu-soft | 大按钮友好；课程卡真实封面图 |
| R-devtool | startup-dark | 代码块真实；深色对比达标 |

---

### 3. 图片填充表（按区块 · 禁占位）

| 区块 | 填什么 | 禁止 |
|------|--------|------|
| Hero | 产品界面 / 场景摄影 / 生成主视觉；有字则半透明遮罩 | placehold 灰块、无 alt 大图 |
| 证据 | 真截图、真数据图、Logo 墙（可单色） | 假仪表盘糊图 |
| 能力卡 | **开源线标** 或小插画 | emoji 图标、每卡随机网图 |
| 作品格 | 项目成片 | 空框「作品」 |
| 叙事/story | 与 kit 色温一致的连续影像 | 赛博霓虹撞山水 |
| 商详 | 多角度实物/生成商品图 | 单张拉伸糊图 |
| 后台 | 基本不插营销图 | 全页 Banner |

来源优先级：① 用户/品牌素材 ② 图库可溯源 URL（Unsplash/Pexels/Pixabay）③ AI 生成落盘 `images/` ④ SVG 插画（仅装饰/线标/能力卡；**不得**单独顶电商商品 packshot /「真实感商品图」成功标准）。  
**缺图默认动作**：搜索图库或生成 → **下载落盘** → 改引用路径 → 预览确认；禁止改验收定义过关。  
**一律**：有意义 `alt`；门禁 G7 无占位服务；计划意图静默降级 = 巡检 **blocking**。

---

### 4. 后端与前端的咬合

| 前端站点类型 | 深度默认 | 后端默认 |
|--------------|----------|----------|
| marketing / story / portfolio / waitlist（无账号） | A | 无或表单 |
| marketing + 注册登录 | B | 轻 API + 一库 |
| app-shell / dashboard / ecommerce 交易 | B/C | 必有；语言见表 backend-architecture |
| 纯静态 SEO | A | 无后端 |

**实现顺序建议（任务大纲）**

1. 信息结构 + 版式变体（可点骨架）  
2. **关键文案**（主 CTA、Hero/屏标题、空错载成功草稿）— 见 `ui-copy-systems.md`  
3. tokens：色 + 字（`:root`）  
4. 真实图填入关键位（Hero/作品）  
5. 动效 light 封顶（除非配方写 brand/3d）  
6. 后端（若需要）与主路径联调  
7. 门禁 + 预览验收  

勿：先上 3D/粒子再补文案结构；A 深度却搭完整 DDD；假字 Lorem 留到上线。

---

### 5. 计划「建议技术」推荐写法（整段可粘）

```text
- 配方：R-overseas | R-ios | R-material | R-fluent | R-ant | R-wechat | R-shanshui | R-tool | …
- 交付深度：A|B|C|D
- 站点类型：…
- 版式变体：Hero=… · 密度=… · …
- 色系 kit：…（字体同 kit；平台 kit 见 color 表 ios-hig/material/…）
- 字体：display=… / body=… / ui=…
- 文案：主 CTA 动词=…；语气跟配方；空/错/载各一句
- 动效档：none|light|brand|3d-hero · 库：…
- 图片：Hero=…；证据=…；来源=图库|生成|品牌
- 后端：无 | 表单服务 | 语言+框架+架构档
- 部署：…
- 为什么：（一句话）
```

### 6. 成功标准（效果向 · 可并入计划）

- [ ] 配方 id 与站点类型一致，无串味  
- [ ] 版式变体 ≥2 维已落地（非纯默认死板三卡）  
- [ ] 色/字 CSS 变量齐全；展示字不进按钮长文  
- [ ] 主 CTA 人话且一致；空/错/载/成功有下一步；无 Lorem/内部 ID 第一句  
- [ ] 关键插图位无占位图；Hero/主图有 alt  
- [ ] 动效不挡 CTA；reduced-motion 可关  
- [ ] 若有后端：深度与接口和主路径一致；A 无企业空壳  
- [ ] `check-landing-gates.sh` 无 FAIL（站点类）  
- [ ] 本地预览主路径 30 秒可讲清  

### 7. 专项真源（深入时再读）

| 主题 | 文件 |
|------|------|
| 布局/变体 | `ui-layout-systems.md` |
| 色 | `ui-color-systems.md` |
| 字 | `ui-typography-systems.md` |
| 界面文案 | `ui-copy-systems.md` |
| 动效 | `ui-motion-effects.md` |
| 后端 | `backend-architecture.md` |
| 门禁 | `landing-gates.md` |
| marketing 节奏 | `examples/marketing-landing-reference/SPEC.md` |
