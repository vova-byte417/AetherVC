#!/bin/bash
# =============================================================================
# AetherVC 持久化存储验证脚本
# 运行环境: WSL Ubuntu 24.04
# 用法: bash scripts/test_persistent_storage.sh
# =============================================================================
set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

FAILURES=()
PROJECT_ROOT="/mnt/d/trae_projects/AetherVC"
TEST_DIR="/tmp/aether_persistent_test"
LOG_DIR="/tmp/aether_test_logs"

rm -rf "$LOG_DIR"
mkdir -p "$LOG_DIR"

# ---------------------------------------------------------------------------
# 工具函数
# ---------------------------------------------------------------------------
add_pass() { :; }

add_fail() {
  FAILURES+=("$1")
}

# 简单检查：命令返回 0 即 PASS
check_silent() {
  local desc="$1"; shift
  echo -n "  [TEST] ${desc} ... "
  if "$@" > /dev/null 2>&1; then
    echo -e "${GREEN}PASS${NC}"
  else
    echo -e "${RED}FAIL${NC}"
    add_fail "${desc}"
  fi
}

# 带日志检查：命令输出保存到文件，失败时打印错误摘要
check_logged() {
  local desc="$1"; local log="$2"; shift 2
  echo -n "  [TEST] ${desc} ... "
  if "$@" > "$log" 2>&1; then
    echo -e "${GREEN}PASS${NC}"
    return 0
  else
    echo -e "${RED}FAIL${NC}"
    add_fail "${desc}"
    return 1
  fi
}

# 检查 cargo test，失败时解析出 FAILED 的测试名
check_cargo_test() {
  local desc="$1"; local log="$2"; shift 2
  echo -n "  [TEST] ${desc} ... "
  if "$@" > "$log" 2>&1; then
    echo -e "${GREEN}PASS${NC}"
    return 0
  else
    echo -e "${RED}FAIL${NC}"
    add_fail "${desc}"
    # 提取失败的测试用例名
    local failed_tests
    failed_tests=$(grep -E "^test .* FAILED$" "$log" | sed 's/ FAILED$//' | sed 's/^test //' || true)
    if [ -n "$failed_tests" ]; then
      echo ""
      echo -e "  ${RED}    失败的测试用例:${NC}"
      while IFS= read -r t; do
        echo -e "  ${RED}      ✗ ${t}${NC}"
      done <<< "$failed_tests"
    fi
    return 1
  fi
}

section() {
  echo ""
  echo -e "${CYAN}━━━ $1 ━━━${NC}"
}

cleanup() { rm -rf "$TEST_DIR"; }

# ---------------------------------------------------------------------------
# 主流程
# ---------------------------------------------------------------------------
trap cleanup EXIT

echo -e "${YELLOW}============================================================${NC}"
echo -e "${YELLOW}  AetherVC 持久化存储功能验证${NC}"
echo -e "${YELLOW}============================================================${NC}"
echo "  日志目录: ${LOG_DIR}"
echo ""

# ─── SECTION 1: 编译 ───
section "1. 编译检查"
cd "$PROJECT_ROOT"

if ! check_logged "aether-core 编译" "$LOG_DIR/build_core.log" cargo build -p aether-core; then
  echo -e "  ${RED}  → 查看日志: cat ${LOG_DIR}/build_core.log${NC}"
fi
if ! check_logged "aether-cli 编译" "$LOG_DIR/build_cli.log" cargo build -p aether-cli; then
  echo -e "  ${RED}  → 查看日志: cat ${LOG_DIR}/build_cli.log${NC}"
fi

# ─── SECTION 2: 单元测试 ───
section "2. 单元测试"

check_cargo_test "aether-core 单元测试" "$LOG_DIR/test_core.log" cargo test -p aether-core
check_cargo_test "aetherci 单元测试"   "$LOG_DIR/test_aetherci.log" cargo test -p aetherci

# ─── SECTION 3: 持久化存储功能验证 ───
section "3. 持久化存储端到端验证"

rm -rf "$TEST_DIR"; mkdir -p "$TEST_DIR"; cd "$TEST_DIR"
git init
git config user.email "test@aethervc.dev"
git config user.name "Test User"
git commit --allow-empty -m "chore: initial commit"
echo "fn main() {}" > main.rs
git add main.rs && git commit -m "feat: add main entry point"
echo "" >> main.rs && git add main.rs && git commit -m "refactor: improve structure"
echo "" >> main.rs && git add main.rs && git commit -m "fix: resolve edge case"
echo "" >> main.rs && git add main.rs && git commit -m "feat: implement user auth"

AETHER_BIN="cargo run --manifest-path $PROJECT_ROOT/aether-cli/Cargo.toml --"

# 初始化
echo "  [INIT] aether init ..."
$AETHER_BIN init 2>&1 || true

# 默认配置检查
echo "  [CONFIG] 检查 storage.backend 默认值 ..."
if $AETHER_BIN config show 2>/dev/null | grep -q 'persistent'; then
  echo -e "  ${GREEN}✓ 默认存储后端为 persistent${NC}"
  add_pass
else
  echo -e "  ${RED}✗ 默认存储后端不是 persistent${NC}"
  add_fail "storage.backend 默认值不是 persistent（期望 persistent）"
  $AETHER_BIN config show 2>/dev/null || true
fi

# 索引
echo "  [INDEX] 构建语义索引 ..."
if $AETHER_BIN index > "$LOG_DIR/index.log" 2>&1; then
  echo -e "  ${GREEN}✓ 索引完成${NC}"
  add_pass
else
  echo -e "  ${RED}✗ 索引失败${NC}"
  add_fail "语义索引失败 (日志: $LOG_DIR/index.log)"
fi

# 向量文件检查
echo ""
echo "  [CHECK] 向量文件持久化:"
VECTOR_COUNT=$(find .aether/vectors -name "*.json" 2>/dev/null | wc -l)
if [ "$VECTOR_COUNT" -gt 0 ]; then
  echo -e "  ${GREEN}✓ .aether/vectors/ 下有 ${VECTOR_COUNT} 个 JSON 文件${NC}"
  ls .aether/vectors/ | head -5
  add_pass
else
  echo -e "  ${RED}✗ .aether/vectors/ 下没有 JSON 文件${NC}"
  add_fail ".aether/vectors/ 下无 JSON 文件，持久化未生效"
fi

# 搜索
echo ""
echo "  [SEARCH] 语义搜索 'user auth' ..."
if $AETHER_BIN search "user auth" -l 5 > "$LOG_DIR/search.log" 2>&1; then
  cat "$LOG_DIR/search.log"
  echo -e "  ${GREEN}✓ 搜索正常返回${NC}"
  add_pass
else
  echo -e "  ${RED}✗ 搜索失败${NC}"
  add_fail "语义搜索失败 (日志: $LOG_DIR/search.log)"
fi

# 状态
echo ""
echo "  [STATUS] aether status ..."
$AETHER_BIN status 2>&1 || true

# 知识图谱
echo ""
echo "  [GRAPH] 知识图谱索引 ..."
if $AETHER_BIN graph index > "$LOG_DIR/graph_index.log" 2>&1; then
  echo -e "  ${GREEN}✓ 知识图谱索引完成${NC}"
  add_pass
else
  echo -e "  ${RED}✗ 知识图谱索引失败${NC}"
  add_fail "知识图谱索引失败 (日志: $LOG_DIR/graph_index.log)"
fi

if [ -f ".aether/graph/graph.json" ]; then
  NODE_COUNT=$(python3 -c "import json; d=json.load(open('.aether/graph/graph.json')); print(len(d.get('nodes',{})))" 2>/dev/null || echo "?")
  echo -e "  ${GREEN}✓ .aether/graph/graph.json 存在 (${NODE_COUNT} 个节点)${NC}"
  add_pass
else
  echo -e "  ${RED}✗ .aether/graph/graph.json 不存在${NC}"
  add_fail ".aether/graph/graph.json 不存在，图存储持久化未生效"
fi

# ─── SECTION 4: 数据不丢失 ───
section "4. 数据重启不丢失验证"

BEFORE_COUNT=$(find .aether/vectors -name "*.json" 2>/dev/null | wc -l)
echo "  索引前向量文件数: ${BEFORE_COUNT}"

$AETHER_BIN index 2>&1 || true

AFTER_COUNT=$(find .aether/vectors -name "*.json" 2>/dev/null | wc -l)
echo "  再次索引后向量文件数: ${AFTER_COUNT}"

if [ "$AFTER_COUNT" -ge "$BEFORE_COUNT" ]; then
  echo -e "  ${GREEN}✓ 持久化数据未丢失 (${BEFORE_COUNT} -> ${AFTER_COUNT})${NC}"
  add_pass
else
  echo -e "  ${RED}✗ 文件数减少! (${BEFORE_COUNT} -> ${AFTER_COUNT})${NC}"
  add_fail "两次索引后向量文件数减少: ${BEFORE_COUNT} -> ${AFTER_COUNT}"
fi

# ─── SECTION 5: 后端切换 ───
section "5. 存储后端切换验证"

echo "  [SWITCH] 切换到内存模式 ..."
if $AETHER_BIN config set storage.backend memory 2>&1; then
  echo -e "  ${GREEN}✓ 配置切换成功${NC}"
  add_pass
else
  echo -e "  ${RED}✗ 配置切换失败${NC}"
  add_fail "config set storage.backend memory 失败"
fi

echo "  [VERIFY] 确认内存模式 ..."
if $AETHER_BIN config show 2>/dev/null | grep -q 'memory'; then
  echo -e "  ${GREEN}✓ 后端已切换为 memory${NC}"
  add_pass
else
  echo -e "  ${RED}✗ 后端未切换为 memory${NC}"
  add_fail "config show 中未找到 memory"
fi

echo "  [INDEX] 内存模式索引 ..."
OUTPUT=$($AETHER_BIN index 2>&1 || true)
if echo "$OUTPUT" | grep -q "内存存储"; then
  echo -e "  ${GREEN}✓ 确认使用内存存储${NC}"
  add_pass
else
  echo "  $OUTPUT"
fi

# 切回 persistent
$AETHER_BIN config set storage.backend persistent 2>&1 || true

echo "  [SEARCH] 切回 persistent 后搜索 ..."
if $AETHER_BIN search "auth" -l 3 > "$LOG_DIR/search_switch.log" 2>&1; then
  echo -e "  ${GREEN}✓ 切回后搜索正常${NC}"
  add_pass
else
  echo -e "  ${RED}✗ 切回后搜索失败${NC}"
  add_fail "切换回 persistent 后搜索失败 (日志: $LOG_DIR/search_switch.log)"
fi

# ─── SECTION 6: 命令行完整性 ───
section "6. CLI 命令完整性"

check_silent "aether --help"          $AETHER_BIN --help
check_silent "aether status"          $AETHER_BIN status
check_silent "aether recover"         $AETHER_BIN recover nonexistent
check_silent "aether analyze --quick" $AETHER_BIN analyze --quick
check_silent "aether digest"          $AETHER_BIN digest
check_silent "aether batch"           $AETHER_BIN batch
check_silent "aether gate status"     $AETHER_BIN gate status
check_silent "aether hook status"     $AETHER_BIN hook status
check_silent "aether config validate" $AETHER_BIN config validate

# ─── 汇总 ───
FAIL_COUNT=${#FAILURES[@]}

section "验证结果汇总"
echo ""

if [ "$FAIL_COUNT" -eq 0 ]; then
  echo -e "  ${GREEN}══════════════════════════════════════════${NC}"
  echo -e "  ${GREEN}  全部验证通过! ✓${NC}"
  echo -e "  ${GREEN}══════════════════════════════════════════${NC}"
  exit 0
fi

echo -e "${RED}══════════════════════════════════════════════════════════════${NC}"
echo -e "${RED}  ${FAIL_COUNT} 项验证未通过:${NC}"
echo -e "${RED}──────────────────────────────────────────────────────────────${NC}"
for i in "${!FAILURES[@]}"; do
  echo -e "  ${RED}  $((i + 1)). ${FAILURES[$i]}${NC}"
done
echo -e "${RED}──────────────────────────────────────────────────────────────${NC}"
echo ""

# 列出有内容的日志文件
echo -e "${YELLOW}  详细日志目录: ${LOG_DIR}${NC}"
for f in "$LOG_DIR"/*.log; do
  [ -f "$f" ] || continue
  [ -s "$f" ] || continue
  echo -e "    ${YELLOW}→ $(basename "$f")${NC}"
done

echo ""
echo -e "${RED}══════════════════════════════════════════════════════════════${NC}"
exit 1
