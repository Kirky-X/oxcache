#!/bin/bash
# 内存泄漏测试脚本
# 支持miri和valgrind两种检测方式

set -e

echo "=== OXCache 内存泄漏检测 ==="

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查是否安装了必要的工具
check_tools() {
    echo "检查必要的工具..."
    
    # 检查miri
    if ! rustup component list --installed | grep -q miri; then
        echo -e "${YELLOW}miri未安装，正在安装...${NC}"
        rustup component add miri
    fi
    
    # 检查valgrind
    if ! command -v valgrind &> /dev/null; then
        echo -e "${YELLOW}valgrind未安装，建议安装以获得更全面的内存检测${NC}"
        echo "Ubuntu/Debian: sudo apt-get install valgrind"
        echo "CentOS/RHEL: sudo yum install valgrind"
        echo "macOS: brew install valgrind"
    fi
    
    echo -e "${GREEN}工具检查完成${NC}"
}

# 使用miri进行内存检测
run_miri_tests() {
    echo -e "\n${YELLOW}=== 运行Miri内存检测 ===${NC}"
    
    # 初始化miri
    cargo miri setup
    
    # 运行内存泄漏测试
    echo "运行内存泄漏测试..."
    MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test memory_leak_test -- --nocapture
    
    echo -e "${GREEN}Miri检测完成${NC}"
}

# 使用valgrind进行内存检测
run_valgrind_tests() {
    echo -e "\n${YELLOW}=== 运行Valgrind内存检测 ===${NC}"
    
    if ! command -v valgrind &> /dev/null; then
        echo -e "${RED}valgrind未安装，跳过valgrind检测${NC}"
        return
    fi
    
    # 构建测试二进制文件
    echo "构建测试二进制文件..."
    cargo test --test memory_leak_test --no-run
    
    # 找到测试二进制文件
    TEST_BINARY=$(find target/debug/deps -name "memory_leak_test-*" -type f -executable | head -1)
    
    if [ -z "$TEST_BINARY" ]; then
        echo -e "${RED}未找到测试二进制文件${NC}"
        return
    fi
    
    echo "使用Valgrind运行内存检测..."
    valgrind --tool=memcheck \
             --leak-check=full \
             --show-leak-kinds=all \
             --track-origins=yes \
             --verbose \
             --log-file=valgrind-memory-leak.log \
             "$TEST_BINARY" --nocapture
    
    # 检查valgrind结果
    if grep -q "definitely lost: 0 bytes" valgrind-memory-leak.log; then
        echo -e "${GREEN}Valgrind检测通过：未发现内存泄漏${NC}"
    else
        echo -e "${RED}Valgrind检测到内存泄漏，请查看 valgrind-memory-leak.log${NC}"
        grep -A 10 "definitely lost" valgrind-memory-leak.log
    fi
}

# 运行内存使用分析
run_memory_analysis() {
    echo -e "\n${YELLOW}=== 运行内存使用分析 ===${NC}"
    
    # 安装内存分析工具
    if ! cargo install --list | grep -q "cargo-mem"; then
        echo "安装cargo-mem工具..."
        cargo install cargo-mem 2>/dev/null || echo "cargo-mem安装失败，跳过"
    fi
    
    # 运行内存测试并收集统计信息
    echo "运行内存测试..."
    cargo test --test memory_leak_test test_l1_cache_memory_leak -- --nocapture
    
    # 使用ps命令监控内存使用
    echo "监控内存使用情况..."
    cargo test --test memory_leak_test test_concurrent_memory_leak &
    TEST_PID=$!
    
    # 监控内存使用
    echo "时间,内存使用(KB)" > memory_usage.csv
    for i in {1..30}; do
        if kill -0 $TEST_PID 2>/dev/null; then
            MEM_USAGE=$(ps -o rss= -p $TEST_PID 2>/dev/null || echo "0")
            echo "$i,$MEM_USAGE" >> memory_usage.csv
            sleep 1
        else
            break
        fi
    done
    
    wait $TEST_PID 2>/dev/null || true
    
    echo -e "${GREEN}内存使用分析完成，数据保存在 memory_usage.csv${NC}"
}

# 生成内存泄漏测试报告
generate_report() {
    echo -e "\n${YELLOW}=== 生成测试报告 ===${NC}"
    
    cat > memory_leak_report.md << EOF
# OXCache 内存泄漏测试报告

## 测试时间
$(date)

## 测试环境
- Rust版本: $(rustc --version)
- 操作系统: $(uname -a)
- 内存: $(free -h | grep Mem | awk '{print $2}')

## 测试结果

### Miri检测结果
- 状态: $(MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test memory_leak_test 2>&1 | grep -q "test result: ok" && echo "✅ 通过" || echo "❌ 失败")

### Valgrind检测结果
EOF

    if [ -f "valgrind-memory-leak.log" ]; then
        echo "- 内存泄漏: $(grep "definitely lost" valgrind-memory-leak.log | tail -1)" >> memory_leak_report.md
        echo "- 间接泄漏: $(grep "indirectly lost" valgrind-memory-leak.log | tail -1)" >> memory_leak_report.md
        echo "- 可能泄漏: $(grep "possibly lost" valgrind-memory-leak.log | tail -1)" >> memory_leak_report.md
    else
        echo "- Valgrind未运行" >> memory_leak_report.md
    fi

    echo "" >> memory_leak_report.md
    echo "## 建议" >> memory_leak_report.md
    echo "- 定期运行内存泄漏检测" >> memory_leak_report.md
    echo "- 在生产环境中监控内存使用" >> memory_leak_report.md
    echo "- 使用jemalloc进行更精确的内存分析" >> memory_leak_report.md
    
    echo -e "${GREEN}测试报告已生成: memory_leak_report.md${NC}"
}

# 主函数
main() {
    cd /home/project/aybss/crates/infra/oxcache
    
    check_tools
    
    # 运行不同的检测方式
    case "${1:-all}" in
        miri)
            run_miri_tests
            ;;
        valgrind)
            run_valgrind_tests
            ;;
        analysis)
            run_memory_analysis
            ;;
        all)
            run_miri_tests
            run_valgrind_tests
            run_memory_analysis
            generate_report
            ;;
        *)
            echo "用法: $0 [miri|valgrind|analysis|all]"
            echo "  miri      - 仅运行Miri检测"
            echo "  valgrind  - 仅运行Valgrind检测"
            echo "  analysis  - 仅运行内存使用分析"
            echo "  all       - 运行所有检测（默认）"
            exit 1
            ;;
    esac
    
    echo -e "\n${GREEN}内存泄漏检测完成！${NC}"
}

# 运行主函数
main "$@"