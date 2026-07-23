# scripts/
> L2 | 父级: /CLAUDE.md

成员清单
package-app.sh: release 构建 cco + cco-desktop → dist/CCO.app；X3 目视清单 + web 标记扫描（模板/写回/拆成步骤/结果台）
smoke.sh: doctor + fake provider 最小冒烟（CCO_STATE_ROOT 隔离）
check-arch.sh: **架构硬规则门禁**（文件行数 · GIANTS 哨兵 · LEGACY_THICK state.js · gateway/invoke · domain 不依赖 tauri）；默认 warn；`STRICT=1` 可失败
check-landing-gates.sh: **落地页/营销站门禁**（example.com · 占位邮箱 · **G7 占位图服务/文件名** · 页脚主 CTA · CTA 刷屏 · h1）；说明 `docs/runtime-prompts/landing-gates.md`；默认 FAIL 硬失败、WARN 不失败；`STRICT=1` WARN 也失败；`SKIP_G1=1`/`SKIP_G7=1` 仅显式演示

## 硬规则

1. `check-arch.sh` 与根 `CLAUDE.md`「工程硬规则」同步；改阈值须改 L1 + 本脚本。  
2. **A5-4**：classic 业务 GIANTS 已空（S8 facade ≤200）；`state.js` 走 LEGACY_THICK 软提醒（**D9+ ~230** · shellUi/statusUi/markdown）；默认非 STRICT。  
3. `check-landing-gates.sh` 与 `docs/runtime-prompts/landing-gates.md` 同步；不扫 `docs/runtime-prompts` 自举举例。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
