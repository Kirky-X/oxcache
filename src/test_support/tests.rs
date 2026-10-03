// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 内嵌假 Redis 服务器语义自测：命令解析、过期回收、通配匹配与 RESP 编码。

use super::*;
use std::io::{BufReader, Write};
use std::net::TcpStream;

fn store() -> Arc<Mutex<Store>> {
    Arc::new(Mutex::new(HashMap::new()))
}

fn run(argv: &[&str], st: &Arc<Mutex<Store>>) -> Vec<Vec<u8>> {
    let argv: Vec<Vec<u8>> = argv.iter().map(|s| s.as_bytes().to_vec()).collect();
    let scripts: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    execute(&argv, st, &scripts)
}

fn str_reply(reply: &[Vec<u8>]) -> String {
    reply
        .iter()
        .flat_map(|b| b.iter())
        .map(|&b| b as char)
        .collect()
}

#[test]
fn resp_replies_are_wellformed() {
    assert_eq!(s_reply("OK"), b"+OK\r\n".to_vec());
    assert_eq!(i_reply(42), b":42\r\n".to_vec());
    assert_eq!(err_reply("bad"), b"-ERR bad\r\n".to_vec());
    assert_eq!(bulk_reply(b"hi"), b"$2\r\nhi\r\n".to_vec());
    assert_eq!(nil_reply(), b"$-1\r\n".to_vec());
}

#[test]
fn glob_match_covers_star_question_and_literal() {
    assert!(glob_match("*", "anything"));
    assert!(glob_match("user:*", "user:42"));
    assert!(!glob_match("user:*", "session:42"));
    assert!(glob_match("k?y", "key"));
    assert!(!glob_match("k?y", "ky"));
    assert!(glob_match("abc", "abc"));
    assert!(!glob_match("abc", "and"));
    // 星号回溯：前缀+星号+后缀
    assert!(glob_match("a*ef", "abcdef"));
    assert!(!glob_match("a*ef", "abcde"));
    assert!(glob_match("**", ""));
    assert!(glob_match("*", ""));
}

#[test]
fn inline_command_without_resp_header_is_parsed() {
    let cursor = std::io::Cursor::new(b"PING\r\n".to_vec());
    let mut reader = BufReader::new(cursor);
    let argv = read_command(&mut reader).unwrap();
    assert_eq!(argv, Some(vec![b"PING".to_vec()]));
    // 空行 → None（连接语义上等价关闭）
    let cursor = std::io::Cursor::new(Vec::new());
    let mut reader = BufReader::new(cursor);
    assert_eq!(read_command(&mut reader).unwrap(), None);
}

#[test]
fn resp_array_command_is_parsed_with_bulk_bodies() {
    let raw = b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$2\r\nvX\r\n".to_vec();
    let cursor = std::io::Cursor::new(raw);
    let mut reader = BufReader::new(cursor);
    let argv = read_command(&mut reader).unwrap().expect("argv");
    assert_eq!(argv, vec![b"SET".to_vec(), b"k".to_vec(), b"vX".to_vec()]);
}

#[test]
fn ping_and_admin_commands_reply_ok() {
    let st = store();
    assert_eq!(str_reply(&run(&["PING"], &st)), "+PONG\r\n");
    assert_eq!(str_reply(&run(&["AUTH", "u", "p"], &st)), "+OK\r\n");
    assert_eq!(str_reply(&run(&["SELECT", "0"], &st)), "+OK\r\n");
    assert_eq!(str_reply(&run(&["CLIENT", "SETNAME", "x"], &st)), "+OK\r\n");
    assert_eq!(
        str_reply(&run(&["NOPE"], &st)),
        "-ERR unknown command 'NOPE'\r\n"
    );
}

#[test]
fn set_get_delete_roundtrip_with_ex_flags() {
    let st = store();
    assert_eq!(str_reply(&run(&["SET", "k", "v"], &st)), "+OK\r\n");
    assert_eq!(str_reply(&run(&["GET", "k"], &st)), "$1\r\nv\r\n");
    assert_eq!(str_reply(&run(&["GET", "missing"], &st)), "$-1\r\n");
    // PX/EX/NX 组合
    assert_eq!(
        str_reply(&run(&["SET", "a", "1", "PX", "5000"], &st)),
        "+OK\r\n"
    );
    assert_eq!(
        str_reply(&run(&["SET", "b", "1", "EX", "5"], &st)),
        "+OK\r\n"
    );
    assert_eq!(str_reply(&run(&["SET", "a", "2", "NX"], &st)), "$-1\r\n");
    assert_eq!(str_reply(&run(&["GET", "a"], &st)), "$1\r\n1\r\n");
    assert_eq!(str_reply(&run(&["EXISTS", "a"], &st)), ":1\r\n");
    assert_eq!(str_reply(&run(&["EXISTS", "zz"], &st)), ":0\r\n");
    assert_eq!(str_reply(&run(&["DEL", "a", "b", "zz"], &st)), ":2\r\n");
    assert_eq!(str_reply(&run(&["GET", "a"], &st)), "$-1\r\n");
}

#[test]
fn setex_and_expiry_shape_ttl_output() {
    let st = store();
    assert_eq!(str_reply(&run(&["SETEX", "s", "100", "v"], &st)), "+OK\r\n");
    let ttl = str_reply(&run(&["TTL", "s"], &st));
    assert!(ttl.starts_with(':') && ttl.trim()[1..].parse::<i64>().unwrap() <= 100);
    assert_eq!(str_reply(&run(&["TTL", "nope"], &st)), ":-2\r\n");
    run(&["SET", "persist", "1"], &st);
    assert_eq!(str_reply(&run(&["TTL", "persist"], &st)), ":-1\r\n");
    // EXPIRE/PEXPIRE 命中与未命中
    assert_eq!(str_reply(&run(&["EXPIRE", "persist", "50"], &st)), ":1\r\n");
    assert_eq!(
        str_reply(&run(&["PEXPIRE", "missing", "50"], &st)),
        ":0\r\n"
    );
}

#[test]
fn expired_keys_are_reaped_on_access() {
    let st = store();
    run(&["SET", "gone", "v", "PX", "1"], &st);
    std::thread::sleep(Duration::from_millis(20));
    // live() 在每次访问时回收过期键
    assert_eq!(str_reply(&run(&["GET", "gone"], &st)), "$-1\r\n");
    assert_eq!(str_reply(&run(&["DBSIZE"], &st)), ":0\r\n");
}

#[test]
fn incr_family_counts_numerically() {
    let st = store();
    assert_eq!(str_reply(&run(&["INCR", "n"], &st)), ":1\r\n");
    assert_eq!(str_reply(&run(&["INCRBY", "n", "41"], &st)), ":42\r\n");
    assert_eq!(str_reply(&run(&["DBSIZE"], &st)), ":1\r\n");
}

#[test]
fn flushall_clears_store() {
    let st = store();
    run(&["SET", "k", "v"], &st);
    assert_eq!(str_reply(&run(&["FLUSHALL"], &st)), "+OK\r\n");
    assert_eq!(str_reply(&run(&["DBSIZE"], &st)), ":0\r\n");
    assert_eq!(str_reply(&run(&["FLUSHDB"], &st)), "+OK\r\n");
}

#[test]
fn keys_and_scan_match_glob_pattern() {
    let st = store();
    run(&["SET", "user:1", "a"], &st);
    run(&["SET", "user:2", "b"], &st);
    run(&["SET", "sess:1", "c"], &st);
    let keys = str_reply(&run(&["KEYS", "user:*"], &st));
    assert!(keys.contains("user:1") && keys.contains("user:2") && !keys.contains("sess:1"));
    let scan = str_reply(&run(&["SCAN", "0", "MATCH", "sess:*"], &st));
    assert!(scan.contains("sess:1") && !scan.contains("user:1"));
}

#[test]
fn info_returns_memory_section_by_default() {
    let st = store();
    let info = str_reply(&run(&["INFO", "memory"], &st));
    assert!(info.contains("used_memory:2048"));
    let clients = str_reply(&run(&["INFO", "clients"], &st));
    assert!(clients.contains("connected_clients:7"));
}

#[test]
fn eval_dispatches_known_script_families() {
    let st = store();
    // 未知脚本 → return 1
    let unknown = run(&["EVAL", "return 7", "1", "k"], &st);
    assert_eq!(str_reply(&unknown), ":1\r\n");
    // INCRBY + PEXPIRE 锁获取脚本
    let acq = run(&["EVAL", "INCRBY", "1", "lock", "3", "5000"], &st);
    assert_eq!(str_reply(&acq), ":3\r\n");
    assert!(
        str_reply(&run(&["TTL", "lock"], &st)).trim()[1..]
            .parse::<i64>()
            .unwrap()
            <= 5
    );
    // CAS 带期望值（GET 家族回退分支）
    let cas_ok = run(&["EVAL", "'GET'", "1", "lock", "3", "9"], &st);
    assert_eq!(str_reply(&cas_ok), ":1\r\n");
    let cas_miss = run(&["EVAL", "'GET'", "1", "lock", "999", "9"], &st);
    assert_eq!(str_reply(&cas_miss), ":0\r\n");
    // 锁释放（DEL 家族）：owner 匹配才删
    let rel_bad = run(&["EVAL", "\"DEL\"", "1", "lock", "not-owner"], &st);
    assert_eq!(str_reply(&rel_bad), ":0\r\n");
    let rel_ok = run(&["EVAL", "\"DEL\"", "1", "lock", "9"], &st);
    assert_eq!(str_reply(&rel_ok), ":1\r\n");
    // CAS 无期望值（EXISTS 家族 set-if-absent）
    let cas_new = run(&["EVAL", "'EXISTS'", "1", "fresh", "v", "5000"], &st);
    assert_eq!(str_reply(&cas_new), ":1\r\n");
    let cas_dup = run(&["EVAL", "'EXISTS'", "1", "fresh", "v2", "5000"], &st);
    assert_eq!(str_reply(&cas_dup), ":0\r\n");
    // 锁续期（PEXPIRE 家族）：owner 匹配才续
    run(&["SET", "lk", "owner1"], &st);
    let renew_bad = run(&["EVAL", "'PEXPIRE'", "1", "lk", "owner2", "1000"], &st);
    assert_eq!(str_reply(&renew_bad), ":0\r\n");
    let renew_ok = run(&["EVAL", "'PEXPIRE'", "1", "lk", "owner1", "1000"], &st);
    assert_eq!(str_reply(&renew_ok), ":1\r\n");
}

#[test]
fn fake_sha1_is_deterministic_40_hex_chars() {
    let a = fake_sha1(b"return 1");
    let b = fake_sha1(b"return 1");
    assert_eq!(a, b);
    assert_eq!(a.len(), 40);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a, fake_sha1(b"return 2"));
}

#[test]
fn ensure_server_serves_ping_over_tcp() {
    // 绑定随机空闲端口，拉起假服务器后用原生 RESP PING 探测
    let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    assert!(ensure_server(port));
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.write_all(b"*1\r\n$4\r\nPING\r\n").expect("write");
    let mut reply = [0u8; 7];
    stream.read_exact(&mut reply).expect("read");
    assert_eq!(&reply, b"+PONG\r\n");
    // 端口已被假服务器占用：再次 ensure_server 走 PING 探测分支返回 true
    assert!(ensure_server(port));
    // 端口被一个不应答 PING 的监听器占住：bind 失败转 PING 探测，
    // 读侧超时（无 accept 无回包）判定不可用返回 false
    let blocker = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let blocked_port = blocker.local_addr().unwrap().port();
    assert!(!ensure_server(blocked_port));
}

#[test]
fn live_reaps_expired_entries_and_keeps_fresh() {
    let mut st: Store = HashMap::new();
    st.insert(
        "old".to_string(),
        (b"1".to_vec(), Some(Instant::now() - Duration::from_secs(1))),
    );
    st.insert("new".to_string(), (b"2".to_vec(), None));
    let kept = live(&mut st);
    assert!(kept.contains_key("new"));
    assert!(!kept.contains_key("old"));
}

// ============================================================================
// `#[cached]` 宏参数解析分支驱动（cache_none / skip / ttl / sync 变体）
//
// 宏在编译期展开执行：这里的使用即覆盖 oxcache_macros 的解析逻辑。
// 错误分支（skip 空参、skip+key 冲突、sync+async、service/ttl 类型错）
// 是 compile_error 路径，由 tests/macros/compile_fail 的 trybuild 用例
// 在独立进程中验证，不进覆盖率。
//
// 用例门控在 `macros` feature 下：宏仅在开启该特性时 re-export 到
// `::oxcache`（invalidation 等单开特性不隐含 macros）。
// ============================================================================

/// cache_none：None 结果也写入哨兵（默认不缓存 None）
#[cfg(feature = "macros")]
#[oxcache::cached(service = "macro_cache_none_self", cache_none)]
async fn macro_cached_none_fn(key: u64) -> Result<Option<String>, String> {
    if key == 0 {
        return Ok(None);
    }
    Ok(Some(format!("v{key}")))
}

/// skip：被排除参数不参与缓存键（相同剩余参数命中同一表项）
#[cfg(feature = "macros")]
#[oxcache::cached(service = "macro_skip_self", skip(secret))]
async fn macro_cached_skip_fn(user: u64, secret: String) -> Result<String, String> {
    let _ = secret;
    Ok(format!("u{user}"))
}

/// ttl：命中窗口注入
#[cfg(feature = "macros")]
#[oxcache::cached(service = "macro_ttl_self", ttl = 30)]
async fn macro_cached_ttl_fn(key: u64) -> Result<String, String> {
    Ok(format!("t{key}"))
}

#[cfg(feature = "macros")]
#[tokio::test]
async fn macro_cached_variants_behave() {
    // cache_none：Some 正常缓存
    assert_eq!(
        macro_cached_none_fn(1).await.unwrap().as_deref(),
        Some("v1")
    );
    assert_eq!(
        macro_cached_none_fn(1).await.unwrap().as_deref(),
        Some("v1")
    );

    // skip：相同 user 不同 secret 命中同一表项
    assert_eq!(macro_cached_skip_fn(7, "a".into()).await.unwrap(), "u7");
    assert_eq!(macro_cached_skip_fn(7, "b".into()).await.unwrap(), "u7");

    // ttl：值往返
    assert_eq!(macro_cached_ttl_fn(3).await.unwrap(), "t3");
}
