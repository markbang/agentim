#!/bin/bash

# CLI Agent 演示脚本
# 展示如何使用CLI Agent与AgentIM系统通讯

set -e

BINARY="./target/release/agentim"

echo "╔════════════════════════════════════════════════════════════╗"
echo "║          AgentIM - CLI Agent 演示                          ║"
echo "║     Interactive Command-Line Communication Demo            ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

echo "🚀 启动交互模式..."
echo ""
echo "在交互模式中，你可以："
echo "  1. 注册CLI Agent"
echo "  2. 注册Channel"
echo "  3. 创建Session"
echo "  4. 发送消息并在CLI中响应"
echo "  5. 查看系统状态"
echo ""
echo "按照菜单提示操作即可。"
echo ""

# 启动交互模式
$BINARY interactive
