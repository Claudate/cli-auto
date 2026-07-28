## 交付与前端（写入相关 do 步骤的 body）

- 若步骤涉及页面/站点/UI：【做什么】写用户可感知结果；【改哪里】写具体目录；【怎样算做完】含主 CTA 可点 + 空/错/载至少各一句验收。
- 绿野站点：静态/SSG 优先；**先**落 `examples/site-floor` 对应 shell+kit（见 `RECIPE-MAP.md` / `demos/r-*`），再换品牌与真图；**禁止**空白页从零猜高端感。brownfield：禁止第二套前端框架。
- 图标：开源线标；禁止 emoji 按钮图标。
- **界面文案**（**正在开发的项目**里用户可见的字：网站/App/软件/后台）：主 CTA 动词人话且全产品一致；空/错/载/成功有下一步；禁 Lorem/TODO/「点击这里」/内部 ID 当用户第一句（`ui-copy-systems.md`）。【怎样算做完】抽查 1 主 CTA + 1 空态 + 1 错误文案。
- **配方对齐**：计划有 `R-*` 时 body 跟 `ui-delivery-recipes` + **site-floor 映射**；顺序优先 **shell/kit 底板**→关键文案→tokens 微调→真图→动效→后端→门禁（`check-landing-gates.sh` 指向站点根）。
- **信息结构**：站点类型 + 必须区块 + 版式变体 2～4；禁止后台套营销五段、作品集空 Hero、无变体死板三卡（`ui-layout-systems`）。
- **图片**：禁占位图；Hero/作品按配方填；alt 必填。计划写「真实感 / 商品图 / 场景图」时，【怎样算做完】**意图 + 代理**两行：① 用户打开货架/Hero 可认作照片级真实感（图库/生成/品牌图）② 且无 placehold 类 host。**缺图默认**：搜索 Unsplash/Pexels/Pixabay 或生成后**下载落盘**并改路径；禁止仅用几何 SVG 顶真实感，禁止把标准改成「非 placehold 即过」。
- **色 / 字 / 动效**：同 kit；动效库≤2；CTA 不挡、reduced-motion。
- **营销站验收**：底 CTA≠footer、无 example.com、顶栏唯一主名、门禁无 FAIL。
- **网页自动化（可选）**：涉及「预览验收 / 截图 / 抓竞品文案 / 表单冒烟」时，可拆 **optional** 步骤，tags 含 `browser`（再加 `ui-verify` | `scrape` | `ui-smoke`）。【怎样算做完】人话；产物约定 `.cco-out/browser/<任务id>/`（shot.png、report.md / raw.md / smoke.md）。抓取必须写 **源 URL + 写入相对路径**，scope 覆盖写入目标。`ui-smoke`：打开 → 填最小必填 → 主 CTA → 成功态，写 smoke.md。默认不强制；宿主 `browser.enabled` 关则 worker 无浏览器工具；结果台可展示证据缩略（见 `docs/browser-automation-cco.md`）。
- **后端**：深度 A–D；A 禁 DDD 空壳；C 骨架须一接口可跑（`backend-architecture`）。
