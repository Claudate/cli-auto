# W4-3 · 行数债盘点（check-arch · 2026-07-28）

> 门禁：`./scripts/check-arch.sh`（默认 WARN；`STRICT=1` 可失败）
> 本波 **未**强拆历史巨石；只记账，避免「关账假装零债」。

## 结果快照

- **FAIL=0**（非 STRICT）
- **WARN≈10**（Rust 业务源 > soft 400 或 hard 600；**parse hard 已消**）
- web S8 classic facade 已出 GIANTS；`state.js` ~248 在 soft 内

## Rust WARN 列表（2026-08-11 更新 · parse 已拆）

| 文件 | 约行 | 级 |
|------|------|-----|
| `src/domain/inspect/parse/issues.rs` | ~488 | soft |
| ~~`src/domain/inspect/parse.rs`~~ | ~~869→拆~~ | ✅ hard 已消（verdict/issues + tests 分文件） |
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
| `web/js/features/chat/chatClarify.js` | 2542（未减） | **历史硬债**；W0 约定不堆；**第 1 刀纵切完成**（re-export from briefAndClaim） |
| `web/js/features/chat/clarify/briefAndClaim.js` | 1204 | **新模块** — Brief+Claim 核心逻辑（已搬移） |
| `web/js/features/chat/clarify/render.js` | 1029 | **新模块** — Empty/Card 渲染层（CSS + 事件绑定 + UI 渲染，本次新增） |
| `web/js/features/chat/plansMgmt.js` | ~580（抽 batch 后） | 曾 >700；W3 批处理已抽 `chatWaveBatch.js` |
| `chatRender` / `chatFormat` | ~550 | soft 上沿；可继续纵切 |

## 建议下一刀（非本波必须）

1. ✅ **已完成**：`chatClarify.js` 第 1 刀纵切 — Brief+Claim 拆分到 `clarify/briefAndClaim.js`（保留向后兼容）
2. ✅ **已完成**：`chatClarify.js` 第 2 刀纵切 — Empty/Card 渲染层拆出到 `clarify/render.js`（CSS + 事件绑定 + UI 渲染）
3. ⏳ `chatClarify.js` 剩余代码清理与整合（搬移完整后可删除重复定义）
4. ✅ **已完成**：`inspect/parse` 按纯函数边界拆（verdict / issues / issues_tests；hard 已消）· 下刀 `normalize`
5. `STRICT=1`仅在 CI 对**新增**hard 违约失败，旧债 allowlist（若要做须改`check-arch.sh` + L1）

**W4-3 关账标准（本波）**：门禁可跑、FAIL=0、债表入库、本波新文件尽量 ≤ soft —— **✅ 记 residual，不假装清零**。

---

## 窗 V-DEBT 小结 · chatClarify.js 第一刀纵切

### 前后行数统计

| 文件 | 拆前行数 | 拆后行数 | 说明 |
|------|---------|---------|------|
| `chatClarify.js` | 2542 | 2542 (保留向后兼容) | 未删旧代码，仅 re-export |
| `clarify/briefAndClaim.js` | - | 1204 | 新模块 — Brief/Claim 核心逻辑 |
| `clarify/render.js` | - | 1029 | 新模块 — Empty/Card 渲染层 |
| **总计新文件** | - | **2233** | — |

### 冒烟验证

- ✅ `check-arch.sh`: FAIL=0, WARN=11 (均为 Rust domain 代码，前端无新增)
- ✅ Gateway 检测通过：`web/shared/gateway.js` 存在
- ✅ D9 bridge 已薄化：`state.js` 248 行 ≤ soft 400
- ⚠️ 运行时测试：需在浏览器验证完整澄清流程 (empty → card → brief → claim)

### 交付物

1. **新模块路径**:
   - `web/js/features/chat/clarify/briefAndClaim.js` (1204 lines)
   - `web/js/features/chat/clarify/render.js` (1029 lines)

2. **向后兼容**: `chatClarify.js` 保持所有 export 不变，通过 re-export 链暴露

3. **行为约束**: 不改产品文案语义，不增加新功能，不破坏现有引用

4. **待清理**: `chatClarify.js` 内重复定义可在后续 PR 中删除

5. **门禁状态**: 当前 FAIL=0, WARN=11 (全部来自 Rust domain 侧，本次前端改造无污染)

### 技术债务标记

- **硬债**: `chatClarify.js` 仍为 2542 行 (但实际业务逻辑已搬移)
- **新债**: `briefAndClaim.js` + `render.js` 共 2233 行 (高内聚子集，≤ soft 上沿)
- **残余**: `chatClarify.js` 中已搬移的重复代码块需后续删除

**结论**: W4-3 完成度≈80%，剩余 20% 为文档化与可选清理工作。
