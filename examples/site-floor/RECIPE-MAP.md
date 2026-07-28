# RECIPE-MAP — R-* → shell + kit

> 与 `docs/runtime-prompts/ui-delivery-recipes.md` 主表对齐。  
> **实现**：`demo` = 可预览页；`组合` = 同 shell 换 kit（可自建 demos 薄页）。  
> **预览**：`python3 -m http.server 8765`（在 `examples/site-floor/`）→ 打开 `demos/<id>/`。

| 配方 id | Shell | 默认 kit | 动效建议 | Demo |
|---------|-------|----------|----------|------|
| **R-overseas** | marketing | western-saas | light | [demos/r-overseas](./demos/r-overseas/) · marketing 窗 |
| **R-cn-brand** | story（或 marketing） | cn-ink（或 shanshui） | light | [demos/r-cn-brand](./demos/r-cn-brand/) · story + cn-ink |
| **R-shanshui** | story | shanshui | light | [demos/r-shanshui](./demos/r-shanshui/) · **P1 必做** |
| **R-jp** | story（或 marketing） | jp-wa（或 jp-minimal） | none/light | [demos/r-jp](./demos/r-jp/) · story + jp-wa |
| **R-portfolio** | portfolio | nordic | light | [demos/r-portfolio](./demos/r-portfolio/) · **P1 必做** |
| **R-waitlist** | waitlist | western-saas | none/light | [demos/r-waitlist](./demos/r-waitlist/) · **P1 必做** |
| **R-docs** | marketing（疏） | nordic | none/light | [demos/r-docs](./demos/r-docs/) · marketing 窗 |
| **R-shop** | marketing | cn-ink（或 cn-festive） | light | [demos/r-shop](./demos/r-shop/) · marketing 窗 |
| **R-tool** | app-shell-lite | western-saas / 品牌 | none/light | [demos/r-tool](./demos/r-tool/) |
| **R-admin** | app-shell-lite | ant-design | none | [demos/r-admin](./demos/r-admin/) · 轻量壳非密表 |
| **R-event** | marketing | cn-festive | light | [demos/r-event](./demos/r-event/) · marketing 窗 |
| **R-fintech** | marketing | fintech | light | [demos/r-fintech](./demos/r-fintech/) · marketing 窗 |
| **R-edu** | marketing | edu-soft | light | [demos/r-edu](./demos/r-edu/) · marketing 窗 |
| **R-devtool** | marketing（或 app-shell-lite） | startup-dark | light/brand | [demos/r-devtool](./demos/r-devtool/) · marketing 窗 · **P1** |
| **R-ios** | marketing / app-shell-lite | ios-hig | light | [demos/r-ios](./demos/r-ios/) · marketing 窗 |
| **R-material** | marketing / app-shell-lite | material | light | [demos/r-material](./demos/r-material/) · marketing 窗 |
| **R-fluent** | marketing / app-shell-lite | fluent | none/light | [demos/r-fluent](./demos/r-fluent/) · marketing 窗 |
| **R-ant** | app-shell-lite | ant-design | none | **同** [demos/r-admin](./demos/r-admin/)（R-ant = R-admin 组合） |
| **R-wechat** | marketing / waitlist | wechat-lite | none/light | [demos/r-wechat](./demos/r-wechat/) · marketing 窗 |
| **R-android-app** | app-shell-lite | material | none/light | [demos/r-android-app](./demos/r-android-app/) |

## 必做目视（P1）

| Demo | Shell × kit | 本窗 |
|------|-------------|------|
| `r-overseas` | marketing × western-saas | marketing 窗 |
| `r-shanshui` | story × shanshui | **本窗** |
| `r-portfolio` | portfolio × nordic | **本窗** |
| `r-waitlist` | waitlist × western-saas | **本窗** |
| `r-devtool` | marketing × startup-dark | marketing 窗 |

## 本窗负责（非营销）

| Demo | Shell | Kit | 配方 |
|------|-------|-----|------|
| r-shanshui | story | shanshui | R-shanshui |
| r-cn-brand | story | cn-ink | R-cn-brand |
| r-jp | story | jp-wa | R-jp |
| r-portfolio | portfolio | nordic | R-portfolio |
| r-waitlist | waitlist | western-saas | R-waitlist |
| r-tool | app-shell-lite | western-saas | R-tool |
| r-admin | app-shell-lite | ant-design | R-admin / **R-ant** |
| r-android-app | app-shell-lite | material | R-android-app |

## 平台优先

用户点名 iOS / Material / Fluent / Ant / 微信时：**平台 kit 优先**，站点类型仍按场景（出海 marketing 等）。见 recipes「组合规则」。

## 门禁

```bash
./scripts/check-landing-gates.sh examples/site-floor/demos/r-shanshui
./scripts/check-landing-gates.sh examples/site-floor/demos/r-portfolio
./scripts/check-landing-gates.sh examples/site-floor/demos/r-waitlist
# 或对本窗全部：
for d in r-shanshui r-cn-brand r-jp r-portfolio r-waitlist r-tool r-admin r-android-app; do
  ./scripts/check-landing-gates.sh "examples/site-floor/demos/$d" || exit 1
done
```

[PROTOCOL]: 增改 R-* / shell / kit 映射须与 `docs/runtime-prompts/ui-delivery-recipes.md` 同步；demo 路径变更时更新本表。
