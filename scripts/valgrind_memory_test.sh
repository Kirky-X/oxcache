#!/bin/bash

# Valgrind内存泄漏检测脚本
# 用于检测Rust代码中的内存泄漏问题

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 默认配置
DEFAULT_BINARY_PATH="target/debug/oxcache"
DEFAULT_TEST_TIMEOUT=300  # 5分钟
DEFAULT_VALGRIND_OPTIONS="--leak-check=full --show-leak-kinds=all --track-origins=yes --verbose"

# 帮助信息
show_help() {
    echo -e "${BLUE}Valgrind内存泄漏检测脚本${NC}"
    echo ""
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  -b, --binary PATH      要测试的二进制文件路径 (默认: $DEFAULT_BINARY_PATH)"
    echo "  -t, --timeout SECONDS  测试超时时间 (默认: $DEFAULT_TEST_TIMEOUT)"
    echo "  -o, --output FILE      输出结果文件"
    echo "  -v, --verbose          详细输出"
    echo "  -h, --help             显示帮助信息"
    echo ""
    echo "示例:"
    echo "  $0                                    # 使用默认配置"
    echo "  $0 -b target/release/oxcache          # 测试release版本"
    echo "  $0 -t 600 -o valgrind_report.txt     # 10分钟超时，输出到文件"
    echo ""
}

# 解析命令行参数
BINARY_PATH="$DEFAULT_BINARY_PATH"
TEST_TIMEOUT="$DEFAULT_TEST_TIMEOUT"
OUTPUT_FILE=""
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -b|--binary)
            BINARY_PATH="$2"
            shift 2
            ;;
        -t|--timeout)
            TEST_TIMEOUT="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo -e "${RED}错误: 未知选项 $1${NC}"
            show_help
            exit 1
            ;;
    esac
done

# 检查依赖
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_dependencies() {
    log_info "检查依赖..."
    
    if ! command -v valgrind &> /dev/null; then
        log_error "Valgrind未安装。请安装Valgrind:"
        log_error "  Ubuntu/Debian: sudo apt-get install valgrind"
        log_error "  CentOS/RHEL: sudo yum install valgrind"
        log_error "  macOS: brew install valgrind"
        exit 1
    fi
    
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo未安装。请安装Rust工具链。"
        exit 1
    fi
    
    log_success "依赖检查通过"
}

# 构建测试二进制文件
build_test_binary() {
    log_info "构建测试二进制文件..."
    
    # 创建测试程序
    cat > /tmp/oxcache_memory_test.rs << 'EOF'
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// 模拟内存泄漏的结构体
struct MemoryLeakTest {
    data: Vec<u8>,
    circular_ref: Option<Arc<Mutex<MemoryLeakTest>>>,
}

impl MemoryLeakTest {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            circular_ref: None,
        }
    }
    
    fn create_circular_reference(&mut self, other: Arc<Mutex<MemoryLeakTest>>) {
        self.circular_ref = Some(other);
    }
}

fn main() {
    println!("开始内存泄漏测试...");
    
    // 测试1: 基本内存分配
    let mut map = HashMap::new();
    for i in 0..1000 {
        let data = vec![i as u8; 1024]; // 1KB per entry
        map.insert(i, data);
    }
    println!("基本内存分配测试完成");
    
    // 测试2: 多线程内存分配
    let handles: Vec<_> = (0..10).map(|i| {
        thread::spawn(move || {
            let mut data = Vec::new();
            for j in 0..100 {
                data.push(vec![i as u8; 1024]);
            }
            thread::sleep(Duration::from_millis(10));
        })
    }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    println!("多线程内存分配测试完成");
    
    // 测试3: 循环引用（潜在的内存泄漏）
    let test1 = Arc::new(Mutex::new(MemoryLeakTest::new(1024)));
    let test2 = Arc::new(Mutex::new(MemoryLeakTest::new(1024)));
    
    {
        let mut t1 = test1.lock().unwrap();
        t1.create_circular_reference(Arc::clone(&test2));
    }
    
    {
        let mut t2 = test2.lock().unwrap();
        t2.create_circular_reference(Arc::clone(&test1));
    }
    
    println!("循环引用测试完成");
    
    // 测试4: 未释放的内存（故意制造的内存泄漏）
    unsafe {
        let _leaked = Box::new(vec![0u8; 10 * 1024 * 1024]); // 10MB
        // 故意不释放，模拟内存泄漏
        Box::into_raw(_leaked);
    }
    println!("内存泄漏测试完成");
    
    // 给Valgrind时间记录内存状态
    thread::sleep(Duration::from_millis(100));
    
    println!("所有测试完成");
}
EOF

    # 编译测试程序
    log_info "编译测试程序..."
    if rustc /tmp/oxcache_memory_test.rs -o /tmp/oxcache_memory_test 2>/dev/null; then
        BINARY_PATH="/tmp/oxcache_memory_test"
        log_success "测试程序编译成功"
    else
        log_warning "测试程序编译失败，使用现有二进制文件"
    fi
}

# 运行Valgrind测试
run_valgrind_test() {
    log_info "运行Valgrind内存泄漏检测..."
    log_info "二进制文件: $BINARY_PATH"
    log_info "超时时间: ${TEST_TIMEOUT}秒"
    
    # 检查二进制文件是否存在
    if [[ ! -f "$BINARY_PATH" ]]; then
        log_error "二进制文件不存在: $BINARY_PATH"
        log_error "请确保已构建项目或使用 -b 指定正确的路径"
        exit 1
    fi
    
    # 准备Valgrind命令
    local valgrind_cmd="valgrind $DEFAULT_VALGRIND_OPTIONS"
    if [[ "$VERBOSE" == true ]]; then
        valgrind_cmd="$valgrind_cmd --verbose"
    fi
    valgrind_cmd="$valgrind_cmd $BINARY_PATH"
    
    # 运行测试
    log_info "执行命令: $valgrind_cmd"
    
    local output
    local exit_code
    
    if [[ -n "$OUTPUT_FILE" ]]; then
        # 输出到文件
        output=$(timeout "$TEST_TIMEOUT" $valgrind_cmd 2>&1)
        exit_code=$?
        echo "$output" > "$OUTPUT_FILE"
    else
        # 直接输出到终端
        timeout "$TEST_TIMEOUT" $valgrind_cmd
        exit_code=$?
    fi
    
    # 分析结果
    analyze_valgrind_output "$output" "$exit_code"
}

# 分析Valgrind输出
analyze_valgrind_output() {
    local output="$1"
    local exit_code="$2"
    
    if [[ $exit_code -eq 124 ]]; then
        log_error "测试超时（${TEST_TIMEOUT}秒）"
        return 1
    elif [[ $exit_code -ne 0 ]]; then
        log_error "Valgrind执行失败（退出码: $exit_code）"
        return 1
    fi
    
    # 检查内存泄漏
    local leak_summary=$(echo "$output" | grep -A 5 "definitely lost:" || true)
    local definitely_lost=$(echo "$leak_summary" | grep "definitely lost:" | awk '{print $4}' || echo "0")
    local indirectly_lost=$(echo "$leak_summary" | grep "indirectly lost:" | awk '{print $4}' || echo "0")
    local possibly_lost=$(echo "$leak_summary" | grep "possibly lost:" | awk '{print $4}' || echo "0")
    local still_reachable=$(echo "$leak_summary" | grep "still reachable:" | awk '{print $4}' || echo "0")
    
    # 移除逗号并转换为数字
    definitely_lost=$(echo "$definitely_lost" | tr -d ',' | tr -d 'B')
    indirectly_lost=$(echo "$indirectly_lost" | tr -d ',' | tr -d 'B')
    possibly_lost=$(echo "$possibly_lost" | tr -d ',' | tr -d 'B')
    still_reachable=$(echo "$still_reachable" | tr -d ',' | tr -d 'B')
    
    # 转换为字节数
    definitely_lost_bytes=$(echo "$definitely_lost" | sed 's/[^0-9]//g')
    indirectly_lost_bytes=$(echo "$indirectly_lost" | sed 's/[^0-9]//g')
    possibly_lost_bytes=$(echo "$possibly_lost" | sed 's/[^0-9]//g')
    still_reachable_bytes=$(echo "$still_reachable" | sed 's/[^0-9]//g')
    
    # 默认值为0
    definitely_lost_bytes=${definitely_lost_bytes:-0}
    indirectly_lost_bytes=${indirectly_lost_bytes:-0}
    possibly_lost_bytes=${possibly_lost_bytes:-0}
    still_reachable_bytes=${still_reachable_bytes:-0}
    
    echo ""
    log_info "内存泄漏检测结果:"
    echo "  Definitely lost: $definitely_lost ($definitely_lost_bytes bytes)"
    echo "  Indirectly lost: $indirectly_lost ($indirectly_lost_bytes bytes)"
    echo "  Possibly lost: $possibly_lost ($possibly_lost_bytes bytes)"
    echo "  Still reachable: $still_reachable ($still_reachable_bytes bytes)"
    echo ""
    
    # 判断测试结果
    local total_lost=$((definitely_lost_bytes + indirectly_lost_bytes))
    
    if [[ $total_lost -eq 0 ]]; then
        log_success "✅ 未检测到内存泄漏！"
        return 0
    elif [[ $total_lost -lt 1024 ]]; then
        log_warning "⚠️  检测到少量内存泄漏: ${total_lost} bytes"
        return 0
    else
        log_error "❌ 检测到严重内存泄漏: ${total_lost} bytes"
        return 1
    fi
}

# 生成报告
generate_report() {
    if [[ -n "$OUTPUT_FILE" ]]; then
        log_info "详细报告已保存到: $OUTPUT_FILE"
        
        # 生成摘要报告
        local summary_file="${OUTPUT_FILE%.txt}_summary.txt"
        cat > "$summary_file" << EOF
Valgrind内存泄漏检测摘要报告
生成时间: $(date)
二进制文件: $BINARY_PATH

$(grep -A 10 "definitely lost:" "$OUTPUT_FILE" | head -n 10)

详细结果请查看: $OUTPUT_FILE
EOF
        
        log_info "摘要报告: $summary_file"
    fi
}

# 主函数
main() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  Valgrind内存泄漏检测工具${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
    
    check_dependencies
    
    # 如果没有指定二进制文件，构建测试程序
    if [[ "$BINARY_PATH" == "$DEFAULT_BINARY_PATH" ]] && [[ ! -f "$BINARY_PATH" ]]; then
        build_test_binary
    fi
    
    run_valgrind_test
    local test_result=$?
    
    generate_report
    
    echo ""
    echo -e "${BLUE}========================================${NC}"
    
    if [[ $test_result -eq 0 ]]; then
        echo -e "${GREEN}内存泄漏检测完成: 通过${NC}"
        exit 0
    else
        echo -e "${RED}内存泄漏检测完成: 失败${NC}"
        exit 1
    fi
}

# 运行主函数
main "$@"