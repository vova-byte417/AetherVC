#!/bin/bash
# =============================================================================
# AetherVC 全量测试脚本
# 运行环境: WSL Ubuntu / 任意 Linux（需 Rust 1.75+）
# 用法:   bash scripts/test_all.sh                # 仅 Mock 测试
#         bash scripts/test_all.sh --real          # 含真实 LLM 集成测试（需 API Key）
#         bash scripts/test_all.sh --full-ci       # CI 模式（更严格）
# =============================================================================
set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BLUE='\033[0;34m'
NC='\033[0m'
BOLD='\033[1m'

REAL_LLM=false
FULL_CI=false
FAILURES=()
START_TIME=$(date +%s)

# ─── 参数解析 ───
for arg in "$@"; do
    case "$arg" in
        --real)   REAL_LLM=true ;;
        --full-ci) FULL_CI=true ;;
    esac
done

# ─── 环境检查 ───
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# 优先使用 rustup 管理的 cargo（~/.cargo/bin/cargo），否则回退到系统 cargo
if [ -x "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi
CARGO="${CARGO:-cargo}"

echo -e "${BOLD}${BLUE}╔══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}${BLUE}║          AetherVC 全量测试套件                             ║${NC}"
echo -e "${BOLD}${BLUE}╚══════════════════════════════════════════════════════════╝${NC}"
echo ""

# Rust 版本检查
RUST_VERSION=$($CARGO --version 2>/dev/null | awk '{print $2}')
if [ -z "$RUST_VERSION" ]; then
    echo -e "${RED}[FATAL] cargo 不可用，请确保 Rust 已安装${NC}"
    exit 1
fi

# 检查 Rust 最低版本要求（1.75+）
RUST_MINOR=$(echo "$RUST_VERSION" | cut -d. -f2)
if [ "${RUST_MINOR:-0}" -lt 75 ] 2>/dev/null; then
    echo -e "${RED}[FATAL] 需要 Rust 1.75+, 当前: $RUST_VERSION${NC}"
    echo -e "${RED}        请通过 rustup 安装更新的工具链: rustup default stable${NC}"
    exit 1
fi
echo -e "  Rust工具链: ${CYAN}$RUST_VERSION${NC}"
echo -e "  项目路径:   ${CYAN}$PROJECT_ROOT${NC}"

if [ "$REAL_LLM" = true ]; then
    if [ -n "${DEEPSEEK_API_KEY:-}" ] || [ -n "${OPENAI_API_KEY:-}" ]; then
        echo -e "  LLM模式:    ${YELLOW}真实 API（已检测到 API Key）${NC}"
    else
        echo -e "  LLM模式:    ${YELLOW}真实 API 模式，但未检测到 DEEPSEEK_API_KEY / OPENAI_API_KEY${NC}"
        echo -e "              ${YELLOW}带 ignore 的真实测试将被跳过${NC}"
    fi
else
    echo -e "  LLM模式:    ${GREEN}Mock（跳过真实 API 测试）${NC}"
fi
echo ""

# ─── 工具函数 ───
add_fail() { FAILURES+=("$1"); }

pass() {
    local desc="$1"; shift
    echo -e "  [${GREEN}PASS${NC}] ${desc}"
}

fail() {
    local desc="$1"; shift
    echo -e "  [${RED}FAIL${NC}] ${desc}"
    add_fail "$desc"
    # 打印额外信息
    if [ $# -gt 0 ]; then
        echo -e "          ${RED}$*${NC}"
    fi
}

run_step() {
    local desc="$1"; local logfile="$2"; shift 2
    echo -n "  [....] ${desc} ... "
    mkdir -p "$(dirname "$logfile")"
    if "$@" > "$logfile" 2>&1; then
        echo -e "\r  [${GREEN}PASS${NC}] ${desc}"
        return 0
    else
        local rc=$?
        echo -e "\r  [${RED}FAIL${NC}] ${desc} (exit=$rc)"
        echo "          --- 末尾 20 行 ---"
        tail -20 "$logfile" | sed 's/^/          | /'
        add_fail "$desc"
        return 1
    fi
}

# ─── 临时日志目录 ───
LOG_DIR="/tmp/aether_test_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$LOG_DIR"
echo "  日志目录: $LOG_DIR"
echo ""

# ══════════════════════════════════════════════════════════════
# Phase 1: 编译检查
# ══════════════════════════════════════════════════════════════
echo -e "${BOLD}${BLUE}── Phase 1: 编译检查 ──${NC}"

cd "$PROJECT_ROOT"

run_step "cargo check (aether-core)"       "$LOG_DIR/check_core.txt"       $CARGO check -p aether-core
run_step "cargo check (aetherci)"           "$LOG_DIR/check_ci.txt"          $CARGO check -p aetherci
run_step "cargo check (aether-cli)"         "$LOG_DIR/check_cli.txt"         $CARGO check -p aether-cli

echo ""

# ══════════════════════════════════════════════════════════════
# Phase 2: aether-core 单元测试
# ══════════════════════════════════════════════════════════════
echo -e "${BOLD}${BLUE}── Phase 2: aether-core 单元测试 ──${NC}"

run_step "aether-core 单元测试"             "$LOG_DIR/test_core_lib.txt"    $CARGO test -p aether-core --lib

# 解析结果
CORE_PASSED=$(grep -oP '\d+(?= passed)' "$LOG_DIR/test_core_lib.txt" 2>/dev/null || echo "?")
CORE_FAILED=$(grep -oP '\d+(?= failed)' "$LOG_DIR/test_core_lib.txt" 2>/dev/null || echo "?")
echo -e "          ${CYAN}=> ${CORE_PASSED} passed, ${CORE_FAILED} failed${NC}"
echo ""

# ══════════════════════════════════════════════════════════════
# Phase 3: aether-core 集成测试
# ══════════════════════════════════════════════════════════════
echo -e "${BOLD}${BLUE}── Phase 3: aether-core 集成测试 ──${NC}"

run_step "集成测试 (workflows)"             "$LOG_DIR/test_integration.txt"  $CARGO test -p aether-core --test workflows

echo ""

# ══════════════════════════════════════════════════════════════
# Phase 4: aetherci 测试
# ══════════════════════════════════════════════════════════════
echo -e "${BOLD}${BLUE}── Phase 4: aetherci 测试 ──${NC}"

run_step "aetherci 单元测试"                "$LOG_DIR/test_ci.txt"          $CARGO test -p aetherci

echo ""

# ══════════════════════════════════════════════════════════════
# Phase 5: 真实 LLM 集成测试（可选）
# ══════════════════════════════════════════════════════════════
if [ "$REAL_LLM" = true ]; then
    echo -e "${BOLD}${BLUE}── Phase 5: 真实 LLM 集成测试 ──${NC}"

    run_step "LLM 集成测试（含 ignored）"    "$LOG_DIR/test_real_llm.txt"    $CARGO test -p aether-core --test real_llm_tests -- --ignored --nocapture

    echo ""
fi

# ══════════════════════════════════════════════════════════════
# Phase 6: CI 模式（格式检查 + 静态分析）
# ══════════════════════════════════════════════════════════════
if [ "$FULL_CI" = true ]; then
    echo -e "${BOLD}${BLUE}── Phase 6: CI 严格检查 ──${NC}"

    run_step "cargo fmt --check"             "$LOG_DIR/fmt.txt"              $CARGO fmt --check --all
    run_step "cargo clippy (aether-core)"     "$LOG_DIR/clippy_core.txt"      $CARGO clippy -p aether-core -- -D warnings
    run_step "cargo clippy (aetherci)"        "$LOG_DIR/clippy_ci.txt"        $CARGO clippy -p aetherci -- -D warnings

    echo ""
fi

# ══════════════════════════════════════════════════════════════
# 结果汇总
# ══════════════════════════════════════════════════════════════
ELAPSED=$(($(date +%s) - START_TIME))

echo -e "${BOLD}${BLUE}╔══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}${BLUE}║          测试汇总                                          ║${NC}"
echo -e "${BOLD}${BLUE}╚══════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ${#FAILURES[@]} -eq 0 ]; then
    echo -e "  ${GREEN}${BOLD}全部通过!${NC}  耗时: ${ELAPSED}s"
    echo ""
    echo "  测试日志: $LOG_DIR"
    exit 0
else
    echo -e "  ${RED}${BOLD}${#FAILURES[@]} 项失败${NC}  耗时: ${ELAPSED}s"
    echo ""
    for ((i=0; i<${#FAILURES[@]}; i++)); do
        echo -e "    ${RED}✗${NC} ${FAILURES[$i]}"
    done
    echo ""
    echo "  完整日志: $LOG_DIR"
    exit 1
fi
