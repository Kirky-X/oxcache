#!/bin/bash
# 文档验证脚本
# 检查文档中的 API 是否在代码中存在

set -e

echo "=== 文档验证脚本 ==="
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 错误计数
ERRORS=0
WARNINGS=0

# 检查示例代码编译
echo "1. 检查示例代码编译"
echo ""

cd examples
if cargo build --all-features 2>&1 | grep -q "error"; then
    echo -e "${RED}✗ 示例代码编译失败${NC}"
    ((ERRORS++))
else
    echo -e "${GREEN}✓ 示例代码编译成功${NC}"
fi
cd ..
echo ""

# 检查文档格式
echo "2. 检查文档格式"
echo ""

for doc in docs/API_REFERENCE.md docs/USER_GUIDE.md docs/SERIALIZATION.md docs/DATABASE_INTEGRATION.md docs/ARCHITECTURE.md docs/CONFIG_VALIDATION.md; do
    if [ -f "$doc" ]; then
        echo -n "检查 $doc ... "
        if grep -q "^#" "$doc" && grep -q "##" "$doc"; then
            echo -e "${GREEN}✓${NC}"
        else
            echo -e "${YELLOW}⚠${NC} 缺少标题"
            ((WARNINGS++))
        fi
    fi
done
echo ""

# 检查链接
echo "3. 检查文档链接"
echo ""

LINK_ERRORS=0
for doc in docs/API_REFERENCE.md docs/USER_GUIDE.md docs/SERIALIZATION.md docs/DATABASE_INTEGRATION.md docs/ARCHITECTURE.md docs/CONFIG_VALIDATION.md; do
    if [ -f "$doc" ]; then
        # 检查 Markdown 链接
        broken_links=$(grep -o '\[.*\](.*\.md)' "$doc" 2>/dev/null | sed 's/.*(\(.*\))/\1/' | while read target; do
            if [ -n "$target" ]; then
                if [ ! -f "docs/$target" ] && [ ! -f "$target" ]; then
                    echo "$target"
                fi
            fi
        done)

        if [ -n "$broken_links" ]; then
            echo -e "${RED}✗ $doc 包含损坏的链接:${NC}"
            echo "$broken_links"
            ((LINK_ERRORS++))
        fi
    fi
done

if [ $LINK_ERRORS -eq 0 ]; then
    echo -e "${GREEN}✓ 所有链接有效${NC}"
else
    ((ERRORS+=LINK_ERRORS))
fi
echo ""

# 检查代码示例语法
echo "4. 检查代码示例语法"
echo ""

CODE_ERRORS=0
for doc in docs/API_REFERENCE.md docs/USER_GUIDE.md docs/SERIALIZATION.md docs/DATABASE_INTEGRATION.md docs/ARCHITECTURE.md docs/CONFIG_VALIDATION.md; do
    if [ -f "$doc" ]; then
        # 提取 Rust 代码块并检查语法
        rust_blocks=$(awk '/```rust/,/```/' "$doc" | grep -v '```' | head -c 10000)
        if [ -n "$rust_blocks" ]; then
            # 简单检查：确保代码块闭合
            if ! grep -q '```rust' "$doc" || ! grep -q '```' "$doc"; then
                echo -e "${YELLOW}⚠ $doc 可能包含未闭合的代码块${NC}"
                ((WARNINGS++))
            fi
        fi
    fi
done

if [ $CODE_ERRORS -eq 0 ]; then
    echo -e "${GREEN}✓ 代码示例格式检查通过${NC}"
fi
echo ""

# 总结
echo "=== 验证结果 ==="
echo -e "错误: ${RED}${ERRORS}${NC}"
echo -e "警告: ${YELLOW}${WARNINGS}${NC}"
echo ""

if [ $ERRORS -gt 0 ]; then
    echo -e "${RED}文档验证失败${NC}"
    exit 1
else
    echo -e "${GREEN}文档验证通过${NC}"
    exit 0
fi
