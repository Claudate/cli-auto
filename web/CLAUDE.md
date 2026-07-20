# web/
> L2 | 父级: /CLAUDE.md

成员清单
index.html: 桌面壳结构；加载 app.css + js/*（D4 顺序 script）；含 page-chat
app.js: 入口说明（逻辑在 js/）
app.css: @import 聚合 css/*（含 chat）
js/: state · flow · plan · monitor · log · chat · doctor（顺序共享全局；flow=流程阶段条 · **stripWorkerScaffold 确认屏只显任务正文**；log=事件过滤/ANSI/导出 MD/虚拟列表 · handoff Board strip · **CLI 日志默认展开**；确认屏删任务/改依赖/引擎；chat C3 多会话+计划 diff+流式 partial）
css/: tokens · layout · plan · monitor · log · chat

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
