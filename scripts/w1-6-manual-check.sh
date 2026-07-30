#!/bin/bash
# W1-6 手动抽检执行脚本
# 用于真人逐项检查并截图

set -euo pipefail

echo "=== W1-6 桌面抽检记录表 ==="
echo ""
echo "1. 启动 CCO.app:"
echo "   $ open /Users/dbi007/project/mac/claude-auto/dist/CCO.app"
echo ""
echo "2. 确保为空项目/新会话状态后，开始以下检查："
echo ""

check() {
    local id=$1
    local desc=$2
    echo "[$id] $desc"
    read -p "通过？(y/n): " resp
    if [[ "$resp" =~ ^[Yy]$ ]]; then
        echo "  → ☑ 过"
    else
        echo "  → ☒ 不过 —— 请截图保存为 /tmp/w1-6-$id-fail.png"
    fi
}

check "A1" "空态无「快试 / 一份计划 / 多需求」三英雄键"
check "A2" "视线落在下方输入 + coach；场景芯片次要"
check "A3" "点「上架详情」→ opener 变电商口吻；不弹职业问卷"
check "A4" "点「制度发文」→ 验收口偏存档确认；与上架可区分"
check "A5" "高级折「范围不对」→ 可折；默认不必点"

echo ""
echo "3. B 区块：含糊三轮边聊"
echo "   步骤：在 #chat-input 依次发送"
echo "   轮 1: 「想做个给客户看的东西，还没想清」"
echo "   轮 2: 「主要给销售用」"
echo "   轮 3: 「先不做登录支付」"
echo ""

check "B1" "发送糊需求后不是满屏考卷大门；可折澄清或短问"
check "B2" "第 2 轮「当前理解」出现/更新「给谁」"
check "B3" "第 3 轮「不做」行更新；假设不装「你已确认」"
check "B4" "扫界面文案 → 首句无 P1–P6、L/M/H 课、run_id、VERDICT"
check "B5" "有草稿后点「按我说的改」→ 焦点回输入；不开跑、不 spawn"

echo ""
echo "4. C 区块：本波多计划"
echo "   步骤：新建会话；输入「本波要日语落地页和英语落地页两件，一起排。」"
echo ""

check "C1" "AI 回复有 wave-index 感或≥2 个计划卡/认领条"
check "C2" "点「认领本波」→ toast 含「未开跑」；跳转管理"
check "C3" "计划列表见「本波 · wave-…」分组；INDEX 不能拆步"
check "C4" "点开详情总览有人话行列；「拆下一份」「确认本波…」可见"
check "C5" "只拆 A；B 的 planned/文件仍在（路径隔离）"
check "C6" "确认本波走闸；同仓一轮后提示再点下批"

echo ""
echo "5. D 区块：红线"

check "D1" "认领/保存/拆步均未静默开跑"
check "D2" "开跑只在确认台/「确认本波」"
check "D3" "optional 任务未被静默勾上（若有）"

echo ""
echo "=== 抽检完成 ==="
echo "请将所有失败项截图保存到 /tmp/w1-6-*.png"
echo "然后回写 docs/path-depth-wave-2026-07-28/w1-6-desktop-checklist.md 和 landing.md"
