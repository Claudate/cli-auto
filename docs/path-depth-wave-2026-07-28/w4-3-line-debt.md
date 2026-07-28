# W4-3 · 行数债盘点（check-arch · 2026-07-28）

> 门禁：`./scripts/check-arch.sh`（默认 WARN；`STRICT=1` 可失败）  
> 本波 **未**强拆历史巨石；只记账，避免「关账假装零债」。

## 结果快照

- **FAIL=0**（非 STRICT）  
- **WARN=11**（Rust 业务源 > soft 400 或 hard 600）  
- web S8 classic facade 已出 GIANTS；`state.js` ~248 在 soft 内  

## Rust WARN 列表（本波未动）

| 文件 | 约行 | 级 |
|------|------|-----|
| `src/domain/inspect/parse.rs` | 869 | hard |
| `src/domain/chat/normalize.rs` | 803 | hard |
| `src/domain/plan/materialize.rs` | 650 | hard |
| `src/domain/chat/clarify.rs` | 623 | hard |
| `src/domain/inspect/residual.rs` | 549 | soft |
| `src/domain/chat/plan_writing_guidance.rs` | 511 | soft |
| `src/domain/plan/cco_split/mod.rs` | 498 | soft |
| `src/app/run/provenance.rs` | 473 | soft |
| `src/domain/worker/cost_budget.rs` | 462 | soft |
| `src/app/run/materialize.rs` | 456 | soft |
| `src/domain/plan/cco_split/humanize.rs` | 401 | soft |

## 本波相关前端

| 文件 | 约行 | 说明 |
|------|------|------|
| `web/js/features/chat/chatClarify.js` | ~2514 | **历史硬债**；W0 约定不堆；大拆另立 |
| `web/js/features/chat/plansMgmt.js` | ~580（抽 batch 后） | 曾 >700；W3 批处理已抽 `chatWaveBatch.js` |
| `chatRender` / `chatFormat` | ~550 | soft 上沿；可继续纵切 |

## 建议下一刀（非本波必须）

1. `chatClarify.js` 按 phase 纵切（empty / card / brief / claim）  
2. `inspect/parse` · `normalize` 按纯函数边界拆文件  
3. `STRICT=1` 仅在 CI 对**新增** hard 违约失败，旧债 allowlist（若要做须改 `check-arch.sh` + L1）

**W4-3 关账标准（本波）**：门禁可跑、FAIL=0、债表入库、本波新文件尽量 ≤ soft —— **✅ 记 residual，不假装清零**。
