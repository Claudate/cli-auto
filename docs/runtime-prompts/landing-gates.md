# 落地页 / 营销站 · 自动门禁说明

> 由 `scripts/check-landing-gates.sh` 执行。  
> 目标：抬高**默认下限**，提高假完成成本；不替代人 30 秒主路径走查。

## 规则（默认）

| ID | 严重度 | 检查 | 说明 |
|----|--------|------|------|
| G1 | FAIL | 用户可见产物含 `example.com` / `app.example.com` | 假域名；文档举例目录可排除 |
| G2 | FAIL | 用户可见产物含 `占位` 邮箱模式 `hello@example.com` / `@example.com` 联系 | 假联系方式 |
| G3 | WARN | `footer` / `site-footer` 块内出现主 CTA 文案（注册领取/立即注册等）且带 primary 按钮类 | 页脚重复主转化 |
| G4 | WARN | 同一 HTML 内主 CTA 文案出现 ≥ 5 次 | 转化按钮刷屏 |
| G5 | FAIL（若存在 index） | 首页无 `<h1` | 缺主标题 |
| G6 | WARN | canonical / og:url 含 example.com | SEO/分享假链 |
| G7 | FAIL | 用户可见源码含占位图服务或占位文件名 | `placehold.co` / `via.placeholder` / `dummyimage` / `placeholder.png` 等；**应用**图库、生成图或品牌实图。仅显式演示可 `SKIP_G7=1` |
| G8 | WARN；有「真实感商品图」意图或 `STRICT=1` 时 FAIL | 电商/真实感语境下商品主图仅为 `/images/products/*.svg`（≥3 且无 jpg/png/webp） | 几何 SVG 不得顶 packshot；缺图应搜图落盘。`SKIP_G8=1` 仅显式演示 |

## 排除路径（不扫或降噪）

- `node_modules/`, `dist/` 可选扫 dist（`SCAN_DIST=1`）
- `docs/runtime-prompts/`（本说明可举 example.com）
- `**/*.md` 默认不扫正文举例（`SCAN_MD=1` 才扫）
- 注释中的说明：脚本主要扫 `.html` `.astro` 及常见前端源码

## 用法

```bash
# 检查当前仓库（默认：web/ 与常见前端根；也可传路径）
./scripts/check-landing-gates.sh
./scripts/check-landing-gates.sh /path/to/site
STRICT=1 ./scripts/check-landing-gates.sh   # WARN 也失败
```

## 与计划验收的关系

计划「成功标准」建议含：

- [ ] `./scripts/check-landing-gates.sh <站点根>` 无 FAIL
- [ ] 人 30 秒：看懂给谁、主按钮唯一可信、点完去哪

仅本地演示时：在计划 **不做/备注** 写明「演示可用 example.com / 临时占位图」，并 `SKIP_G1=1` / `SKIP_G7=1` / `SKIP_G8=1` 显式跳过（不推荐默认）。

### 图片期望（与 G7 / G8 配套 · 写入计划/拆分）

- **允许**：Unsplash / Pexels / Pixabay 等可溯源 URL；项目内 `images/` 下载或 AI 生成文件；用户品牌图。  
- **SVG 插画**：允许用于图标、能力卡、装饰；**当计划成功标准写「真实感商品图 / 场景摄影 / packshot」时，商品主图与 Hero 主视觉不得仅以几何 SVG 插画交差**（缺图 → 搜图落盘）。G8 在电商标记或计划意图下对「商品位全是 `.svg`、无照片扩展名」告警/失败。  
- **禁止**：占位图 CDN、灰块「Image」、无 `alt` 的装饰大图冒充产品图、把「无 placehold host」当作「真实感」的完整验收。
