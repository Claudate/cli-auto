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

仅本地演示时：在计划 **不做/备注** 写明「演示可用 example.com」，并 `SKIP_G1=1` 显式跳过（不推荐默认）。
