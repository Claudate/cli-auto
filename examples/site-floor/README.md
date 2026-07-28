# site-floor — 可运行高端站点底板

> **用途**：绿野网站的**观感下限**。Worker / 聊天写计划时**先对齐或复制本目录**，再换品牌文案与真图。  
> **不是** cco 产品官网生成器；cco 仍是任务控制台（见根目录 `PRODUCT.md`）。  
> **对标机制**（Michael IDE 类）：精选设计系统脚手架 + token/组件，而不是只靠「请做得高端」提示词。

## 怎么用（人话）

1. 在 [`RECIPE-MAP.md`](./RECIPE-MAP.md) 查配方 `R-*` → **shell（骨架）+ kit（色/字）**。  
2. 打开 `demos/<配方>/index.html` 预览，或复制对应 `shells/` + `kits/` + `shared/base.css` 到目标项目。  
3. 只改文案、主名、真图、链接；**不要**从空白另起满屏默认字体 + 随机色。  
4. 做完跑：`./scripts/check-landing-gates.sh examples/site-floor/demos/<id>`

## 预览

```bash
cd examples/site-floor
python3 -m http.server 8765
# 浏览器打开 http://127.0.0.1:8765/demos/r-overseas/
```

## 目录

| 路径 | 含义 |
|------|------|
| `shared/base.css` | 间距、按钮、卡片、动效底线（无品牌色） |
| `kits/*.css` | 与 `docs/runtime-prompts/ui-color-systems` **同名**的色/字 token |
| `shells/*` | 按站点类型的区块骨架（marketing / story / portfolio / waitlist / app-shell-lite） |
| `demos/r-*` | 配方组合可点开页（同 shell 换 kit，避免 N 份重复设计） |
| `RECIPE-MAP.md` | 全表 `R-*` → shell + kit |

## 与文档配方关系

- 决策与组合真源：`docs/runtime-prompts/ui-delivery-recipes.md`  
- 营销区块节奏 reference：`examples/marketing-landing-reference/SPEC.md`  
- 门禁：`scripts/check-landing-gates.sh` · `docs/runtime-prompts/landing-gates.md`  
- **注入 LLM 的是 md 纪律**（要求用本底板）；**本目录本身不整包注入**上下文。

## 纪律（反 AI 土味）

- 颜色只进 kit 变量；组件用 `.btn` / `.card`。  
- 图标：线标 SVG；**禁止** emoji 按钮。  
- 底 CTA 带与 Footer **必须分色**（marketing）。  
- 演示可用色块示意；上线图库/生成图落盘，禁 placehold 类 host。  
- 绿野任务顺序：结构（本 shell）→ 文案 → tokens（本 kit）→ 真图 → 动效 → 后端 → 门禁。

[PROTOCOL]: 增改 kit 名须与 ui-color / ui-typography 对齐；新 shell 须更新 RECIPE-MAP 与 examples/CLAUDE.md
