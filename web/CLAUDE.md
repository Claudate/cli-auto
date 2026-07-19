# web/
> L2 | 父级: /CLAUDE.md

成员清单
index.html: 桌面壳结构；加载 app.css + js/*（D4 顺序 script）；含 page-chat
app.js: 入口说明（逻辑在 js/）
app.css: @import 聚合 css/*（含 chat）
js/: state · plan · monitor · log · chat · doctor（顺序共享全局）
css/: tokens · layout · plan · monitor · log · chat

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
