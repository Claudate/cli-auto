# 计划 05 · 模块化拆分 · 运行方式 · 省钱 CLI · 验收

> 本波：[INDEX.md](./INDEX.md) · 落地：[landing.md](./landing.md)  
> 问题：拆分如何模块化（一步挂了不炸全场）？如何讲清并行与省钱跑？如何对照验收？  
> 状态：战略 · W2–W3 · **大量执行能力已存在，重在组合与人话**  

---

## 目标

1. **拆分模块化**：重试边界至少到「单份计划」；Bundle 内一份 `plan_failed` 不取消其它。  
2. **运行方式可见**：每计划几任务、哪波可并、计划谁先谁后、思考/执行/验收分工人话。  
3. **省钱**：复用 cost-route（implement 偏中低、inspect/拆分思考偏旗舰）；主路径不堆引擎名。  
4. **验收**：per-plan 对照计划 + 本波齐活小结；完成 ≠ 进程 exit 0。  

## 非目标

- 重写 Scheduler  
- 默认同 worktree 多计划真并行写码  
- 主路径展示 VERDICT/run_id  
- 无闸全自动连跑  

## 模块化拆分

```text
Bundle.split
  ├── Plan A  ✅
  ├── Plan B  ❌ → 只重拆 B（可带 revision 意见）
  └── Plan C  ⏳
```

| 层 | 已有/要补 |
|----|-----------|
| 任务执行失败重试/回补 | 已有方向，保持 |
| 单计划 replan/preserve | 已有，保持 |
| 计划级失败隔离 + 按 path supersede | **要补** |
| 拆分中单波任务生成失败不丢已成功任务 | 第二刀，后置 |

## 并行两层（务必分清）

| 层 | 含义 | cco |
|----|------|-----|
| 任务并行 | 同计划内无依赖多窗口 | ✅ max_parallel / waves |
| 计划并行 | 多份计划同时推进 | 总览可标 parallel_ok；**执行默认串行**；高级+隔离才真并 |

同仓无隔离真并行 = 冲突源；与 multi-cli「同项目多 run 慎用」一致。

## 省钱与角色（复用）

| 角色倾向 | tier（已有 cost-route） |
|----------|-------------------------|
| scout | cheap |
| implement | mid（可降） |
| integrate / inspect | flagship（不降） |
| 拆分/思考 | 旗舰倾向（plan agent） |

确认屏人话示例：

> 写代码多用较省的工具；**怎么拆开、最后有没有做完**用更强的。

## 组合总览（用户看见的排版）

```text
本波 · 3 份计划     [确认本波]
可一起干：① ②　先做完再做：③（等 ①）

① 日语落地页  8 步 · 最多 3 路并行
   W1 …  W2 …  W3 验收
② …  5 步
③ …  等待
```

概念 ≤3：**几份计划 · 能否一起干 · 确认本波**。

## 验收闭环

- 每计划：对照该 plan 巡检 / Ensure 能力复用  
- 本波：索引级「是否齐」短结论  
- 有漏：回补波挂在**出错计划**上，不默认整 bundle 重做  

## 现状锚

| ✅ | 缺口 |
|----|------|
| 拆分台波次、Scheduler、worktree、cost-route、inspect/Ensure | Bundle 总览 DTO、按 path 拆隔离、计划默认串行编排、人话费用条 |

## 验收（W2–W3）

- [x] 两计划：B 拆失败 / 重拆不杀 A（W2-4 supersede per path）  
- [x] 总览含计划数/步数/状态与串行人话（W3 `chatWaveOverview`）  
- [x] confirm 不旁路（批确认仍 `confirm_start`）；optional 不静默  
- [x] implement 默认非旗舰、inspect 旗舰（既有 cost-route 测）  
- [x] 主路径无引擎名第一句  

## 依赖

硬依赖 [04-multi-plan-bundle.md](./04-multi-plan-bundle.md) 的 Bundle/索引心智。任务并行展示可先于 Bundle 代码在单计划打磨。

## 修订

2026-07-28 初版拆出
