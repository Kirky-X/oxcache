// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 基于测试内嵌 RESP2 假 Redis 服务器的 Redis 族后端覆盖测试。
//!
//! `RedisBackend` / `DragonflyBackend` / `DistributedLock` 的完整命令路径
//! 此前只被容器集成测试覆盖（valkey/dragonfly 镜像拉取受限的环境下整体
//! 跳过）。本文件在测试进程内拉起一个最小 RESP2 协议服务器（std 线程 +
//! 阻塞 IO），覆盖 async/sync trait 全操作、pipeline、namespace、Lua 执行、
//! 断路器、分布式锁生命周期与 watchdog——不依赖任何外部服务或容器。
//!
//! 假服务器只实现本项目实际下发命令的语义（GET/SET/DEL/EXISTS/PEXPIRE/
//! TTL/DBSIZE/SCAN/INFO/INCR/SCRIPT/EVAL 等）；EVAL 不执行 Lua，按本项目
//! 已知脚本的关键字分发到等价原生语义（INCRBY+PEXPIRE / 比较后 DEL /
//! 比较后 PEXPIRE / SET-if-absent / CAS）。

#![cfg(all(
    feature = "redis",
    feature = "lock",
    feature = "dragonfly",
    feature = "lua"
))]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use oxcache::backend::memory::RedisBackendBuilder;
use oxcache::backend::memory::redis::RedisBackend;
use oxcache::backend::{
    AtomicCacheWriter, BackendKind, BackendScore, CacheConnector, CacheReader, CacheSetItem,
    CacheWriter, LuaExecutor, Scores,
};
use oxcache::error::OxCacheResult;
use oxcache::features::dist_lock::{DistLockBuilder, LockProvider};

// libtest 默认多线程并发运行测试，POSIX `environ` 非线程安全；进程加载期
// （main 前、单线程期）由 ctor 一次性写入，取值与 builder 白名单字面量一致，
// 同值幂等。与 tests/common/mod.rs 同口径。
#[ctor::ctor(unsafe)]
fn init_allow_insecure_redis_env() {
    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    }
}

// ============================================================================
// 假 Redis 服务器
// ============================================================================

type Store = HashMap<String, (Vec<u8>, Option<Instant>)>;

struct FakeServer {
    store: Arc<Mutex<Store>>,
    /// 置位后所有命令返回 -ERR（断路器 / watchdog 错误分支驱动用）。
    fail_all: Arc<AtomicBool>,
}

impl FakeServer {
    fn spawn() -> (Arc<Self>, String) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().expect("local addr").port();
        // 非阻塞 accept 轮询；测试进程退出时线程随进程一并终止。
        listener
            .set_nonblocking(true)
            .expect("set nonblocking listener");
        let store: Arc<Mutex<Store>> = Arc::new(Mutex::new(HashMap::new()));
        let scripts: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let fail_all = Arc::new(AtomicBool::new(false));

        let accept_store = Arc::clone(&store);
        let accept_scripts = Arc::clone(&scripts);
        let accept_fail = Arc::clone(&fail_all);
        std::thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let conn_store = Arc::clone(&accept_store);
                        let conn_scripts = Arc::clone(&accept_scripts);
                        let conn_fail = Arc::clone(&accept_fail);
                        std::thread::spawn(move || {
                            serve_connection(stream, conn_store, conn_scripts, conn_fail)
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });

        let server = Arc::new(Self { store, fail_all });
        (server, format!("redis://127.0.0.1:{port}"))
    }

    fn set_fail_all(&self, on: bool) {
        self.fail_all.store(on, Ordering::SeqCst);
    }

    fn contains_key(&self, key: &str) -> bool {
        Self::live(&mut self.store.lock().expect("store lock")).contains_key(key)
    }

    fn remove_key(&self, key: &str) {
        Self::live(&mut self.store.lock().expect("store lock")).remove(key);
    }

    /// 惰性过期：访问前剔除已到期条目。
    fn live(store: &mut Store) -> &mut Store {
        let now = Instant::now();
        store.retain(|_, (_, expires)| expires.is_none_or(|at| at > now));
        store
    }
}

async fn backend_on(url: &str) -> RedisBackend {
    RedisBackend::builder()
        .connection_string(url)
        .retry_count(0)
        .build()
        .await
        .expect("connect to fake redis")
}

async fn connected_backend() -> (Arc<FakeServer>, Arc<RedisBackend>) {
    let (server, url) = FakeServer::spawn();
    let backend = backend_on(&url).await;
    (server, Arc::new(backend))
}

fn fake_sha1(script: &[u8]) -> String {
    // FNV-1a 64 位散列，零填充至 40 位十六进制——只需在本进程内对同一脚本
    // 稳定且足以充当 EVALSHA 缓存键。
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in script {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}").repeat(5)[..40].to_string()
}

fn glob_match(pattern: &str, text: &str) -> bool {
    // 迭代双指针 + 回溯（与 src/backend/interface.rs::glob_match 同算法）。
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_pi, mut star_ti) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn serve_connection(
    stream: TcpStream,
    store: Arc<Mutex<Store>>,
    scripts: Arc<Mutex<HashMap<String, String>>>,
    fail_all: Arc<AtomicBool>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_nodelay(true);
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut out = stream;

    while let Ok(Some(argv)) = read_command(&mut reader) {
        if argv.is_empty() {
            continue;
        }
        if fail_all.load(Ordering::SeqCst) {
            if out.write_all(b"-ERR forced failure for test\r\n").is_err() {
                break;
            }
            let _ = out.flush();
            continue;
        }
        let replies = execute(&argv, &store, &scripts);
        let mut buf: Vec<u8> = Vec::new();
        for reply in replies {
            buf.extend_from_slice(&reply);
        }
        if out.write_all(&buf).is_err() {
            break;
        }
        let _ = out.flush();
    }
}

/// 读取一条 RESP2 请求（批量字符串数组）。连接关闭返回 Ok(None)。
fn read_command(reader: &mut BufReader<TcpStream>) -> std::io::Result<Option<Vec<Vec<u8>>>> {
    let mut header = Vec::new();
    if reader.read_until(b'\n', &mut header)? == 0 {
        return Ok(None);
    }
    let header = String::from_utf8_lossy(&header);
    let header = header.trim_end();
    let Some(stripped) = header.strip_prefix('*') else {
        // 内联命令兜底：按空白切分。
        let parts: Vec<Vec<u8>> = header
            .split_whitespace()
            .map(|s| s.as_bytes().to_vec())
            .collect();
        return Ok(if parts.is_empty() { None } else { Some(parts) });
    };
    let count: usize = stripped.parse().unwrap_or(0);
    let mut argv = Vec::with_capacity(count);
    for _ in 0..count {
        let mut len_line = Vec::new();
        if reader.read_until(b'\n', &mut len_line)? == 0 {
            return Ok(None);
        }
        let len_line = String::from_utf8_lossy(&len_line);
        let len: usize = len_line
            .trim_end()
            .strip_prefix('$')
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let mut body = vec![0u8; len + 2]; // 含尾部 CRLF
        reader.read_exact(&mut body)?;
        body.truncate(len);
        argv.push(body);
    }
    Ok(Some(argv))
}

fn s_reply(text: &str) -> Vec<u8> {
    format!("+{text}\r\n").into_bytes()
}

fn i_reply(n: i64) -> Vec<u8> {
    format!(":{n}\r\n").into_bytes()
}

fn err_reply(msg: &str) -> Vec<u8> {
    format!("-ERR {msg}\r\n").into_bytes()
}

fn bulk_reply(bytes: &[u8]) -> Vec<u8> {
    let mut out = format!("${}\r\n", bytes.len()).into_bytes();
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\r\n");
    out
}

fn nil_reply() -> Vec<u8> {
    b"$-1\r\n".to_vec()
}

/// 单命令执行，返回零或多条响应（pipeline 逐条响应）。
fn execute(
    argv: &[Vec<u8>],
    store: &Arc<Mutex<Store>>,
    scripts: &Arc<Mutex<HashMap<String, String>>>,
) -> Vec<Vec<u8>> {
    let cmd = String::from_utf8_lossy(&argv[0]).to_uppercase();
    let args = &argv[1..];
    let arg = |i: usize| -> String {
        args.get(i)
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default()
    };

    match cmd.as_str() {
        "PING" => vec![s_reply("PONG")],
        "AUTH" | "SELECT" => vec![s_reply("OK")],
        "CLIENT" => vec![s_reply("OK")],
        "FLUSHALL" | "FLUSHDB" => {
            store.lock().expect("store lock").clear();
            vec![s_reply("OK")]
        }
        "DBSIZE" => {
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            vec![i_reply(st.len() as i64)]
        }
        "GET" => {
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            match st.get(&arg(0)) {
                Some((v, _)) => vec![bulk_reply(v.as_slice())],
                None => vec![nil_reply()],
            }
        }
        "SET" => {
            let key = arg(0);
            let value = args.get(1).cloned().unwrap_or_default();
            let mut px_ms: Option<u64> = None;
            let mut nx = false;
            let mut i = 2;
            while i < args.len() {
                let flag = String::from_utf8_lossy(&args[i]).to_uppercase();
                match flag.as_str() {
                    "PX" => {
                        px_ms = arg(i + 1).parse().ok();
                        i += 2;
                    }
                    "EX" => {
                        px_ms = arg(i + 1).parse::<u64>().ok().map(|s| s * 1000);
                        i += 2;
                    }
                    "NX" => {
                        nx = true;
                        i += 1;
                    }
                    _ => i += 1,
                }
            }
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            if nx && st.contains_key(&key) {
                return vec![nil_reply()];
            }
            let expires = px_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
            st.insert(key, (value, expires));
            vec![s_reply("OK")]
        }
        "SETEX" => {
            let secs: u64 = arg(1).parse().unwrap_or(0);
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            st.insert(
                arg(0),
                (
                    args.get(2).cloned().unwrap_or_default(),
                    Some(Instant::now() + Duration::from_secs(secs)),
                ),
            );
            vec![s_reply("OK")]
        }
        "DEL" => {
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            let mut removed = 0i64;
            for k in args {
                if st
                    .remove(&String::from_utf8_lossy(k).into_owned())
                    .is_some()
                {
                    removed += 1;
                }
            }
            vec![i_reply(removed)]
        }
        "EXISTS" => {
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            let key = arg(0);
            vec![i_reply(i64::from(st.contains_key(&key)))]
        }
        "PEXPIRE" | "EXPIRE" => {
            let key = arg(0);
            let ttl: u64 = arg(1).parse().unwrap_or(0);
            let ms = if cmd == "PEXPIRE" { ttl } else { ttl * 1000 };
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            match st.get_mut(&key) {
                Some((_, expires)) => {
                    *expires = Some(Instant::now() + Duration::from_millis(ms));
                    vec![i_reply(1)]
                }
                None => vec![i_reply(0)],
            }
        }
        "TTL" | "PTTL" => {
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            let key = arg(0);
            match st.get(&key) {
                None => vec![i_reply(-2)],
                Some((_, None)) => vec![i_reply(-1)],
                Some((_, Some(at))) => {
                    let ms = at.saturating_duration_since(Instant::now()).as_millis() as i64;
                    let n = if cmd == "PTTL" { ms } else { (ms + 999) / 1000 };
                    vec![i_reply(n)]
                }
            }
        }
        "INCR" => vec![incr_by(store, &arg(0), 1)],
        "INCRBY" => vec![incr_by(store, &arg(0), arg(1).parse().unwrap_or(0))],
        "SCAN" => {
            let pattern = args
                .windows(2)
                .find(|w| w[0].eq_ignore_ascii_case(b"MATCH"))
                .map(|w| String::from_utf8_lossy(&w[1]).into_owned())
                .unwrap_or_else(|| "*".to_string());
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            let keys: Vec<String> = st
                .keys()
                .filter(|k| glob_match(&pattern, k))
                .cloned()
                .collect();
            // 单批返回全部命中，游标归零终止客户端 SCAN 循环。
            let mut reply = b"*2\r\n$1\r\n0\r\n".to_vec();
            reply.extend_from_slice(format!("*{}\r\n", keys.len()).as_bytes());
            for k in &keys {
                reply.extend_from_slice(&bulk_reply(k.as_bytes()));
            }
            vec![reply]
        }
        "KEYS" => {
            let pattern = arg(0);
            let mut st = store.lock().expect("store lock");
            FakeServer::live(&mut st);
            let keys: Vec<String> = st
                .keys()
                .filter(|k| glob_match(&pattern, k))
                .cloned()
                .collect();
            let mut reply = format!("*{}\r\n", keys.len()).into_bytes();
            for k in &keys {
                reply.extend_from_slice(&bulk_reply(k.as_bytes()));
            }
            vec![reply]
        }
        "INFO" => {
            let section = arg(0).to_lowercase();
            let body = if section == "clients" {
                "connected_clients:7\r\nmaxclients:10000\r\n".to_string()
            } else {
                "used_memory:2048\r\nused_memory_human:2K\r\n".to_string()
            };
            vec![bulk_reply(body.as_bytes())]
        }
        "SCRIPT" => {
            // SCRIPT LOAD <script>
            let script = args.get(2).cloned().unwrap_or_default();
            let sha = fake_sha1(&script);
            scripts
                .lock()
                .expect("scripts lock")
                .insert(sha.clone(), String::from_utf8_lossy(&script).into_owned());
            vec![bulk_reply(sha.as_bytes())]
        }
        "EVALSHA" => {
            let sha = arg(0);
            let loaded = scripts.lock().expect("scripts lock").contains_key(&sha);
            if loaded {
                vec![i_reply(1)]
            } else {
                vec![b"-NOSCRIPT No matching script. Please use EVAL.\r\n".to_vec()]
            }
        }
        "EVAL" => vec![eval_known_script(store, argv)],
        _ => vec![err_reply(&format!("unknown command '{cmd}'"))],
    }
}

fn script_matches_known_family(script: &str) -> bool {
    script.contains("INCRBY")
        || script.contains("'DEL'")
        || script.contains("\"DEL\"")
        || script.contains("'EXISTS'")
        || script.contains("\"EXISTS\"")
        || script.contains("'PEXPIRE'")
        || script.contains("\"PEXPIRE\"")
        || script.contains("'GET'")
        || script.contains("\"GET\"")
}

fn incr_by(store: &Arc<Mutex<Store>>, key: &str, delta: i64) -> Vec<u8> {
    let mut st = store.lock().expect("store lock");
    FakeServer::live(&mut st);
    let current: i64 = st
        .get(key)
        .and_then(|(v, _)| String::from_utf8_lossy(v).parse().ok())
        .unwrap_or(0);
    let next = current + delta;
    st.insert(key.to_string(), (next.to_string().into_bytes(), None));
    i_reply(next)
}

fn apply_pexpire(store: &Arc<Mutex<Store>>, key: &str, ttl_ms: u64) {
    let mut st = store.lock().expect("store lock");
    if let Some((_, expires)) = st.get_mut(key) {
        *expires = Some(Instant::now() + Duration::from_millis(ttl_ms));
    }
}

/// EVAL 分发：按本项目已知脚本的关键字映射到等价原生语义。
fn eval_known_script(store: &Arc<Mutex<Store>>, argv: &[Vec<u8>]) -> Vec<u8> {
    if !script_matches_known_family(&String::from_utf8_lossy(&argv[1])) {
        // 未知脚本（测试自造）：等价 return 1
        return i_reply(1);
    }
    let script = String::from_utf8_lossy(&argv[1]).into_owned();
    let key = argv
        .get(3)
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    let args: Vec<Vec<u8>> = argv.iter().skip(4).cloned().collect();

    if script.contains("INCRBY") {
        // incr + PEXPIRE 原子脚本
        let delta: i64 = args
            .first()
            .map(|b| String::from_utf8_lossy(b).parse().unwrap_or(0))
            .unwrap_or(0);
        let ttl_ms: u64 = args
            .get(1)
            .map(|b| String::from_utf8_lossy(b).parse().unwrap_or(0))
            .unwrap_or(0);
        let reply = incr_by(store, &key, delta);
        apply_pexpire(store, &key, ttl_ms);
        reply
    } else if script.contains("'DEL'") || script.contains("\"DEL\"") {
        // 锁释放脚本：GET 比对 owner，匹配才 DEL
        let owner = args.first().cloned().unwrap_or_default();
        let mut st = store.lock().expect("store lock");
        FakeServer::live(&mut st);
        match st.get(&key) {
            Some((v, _)) if v.as_slice() == owner.as_slice() => {
                st.remove(&key);
                i_reply(1)
            }
            _ => i_reply(0),
        }
    } else if script.contains("'EXISTS'") || script.contains("\"EXISTS\"") {
        // CAS 无期望值（set-if-absent）：ARGV[1]=new [ARGV[2]=PX]
        let new = args.first().cloned().unwrap_or_default();
        let px_ms: Option<u64> = args
            .get(1)
            .and_then(|b| String::from_utf8_lossy(b).parse().ok());
        let mut st = store.lock().expect("store lock");
        FakeServer::live(&mut st);
        match st.entry(key) {
            std::collections::hash_map::Entry::Occupied(_) => i_reply(0),
            std::collections::hash_map::Entry::Vacant(v) => {
                let expires = px_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
                v.insert((new, expires));
                i_reply(1)
            }
        }
    } else if script.contains("'PEXPIRE'") || script.contains("\"PEXPIRE\"") {
        // 锁续期脚本：GET 比对 owner，匹配才 PEXPIRE
        let owner = args.first().cloned().unwrap_or_default();
        let ttl_ms: u64 = args
            .get(1)
            .map(|b| String::from_utf8_lossy(b).parse().unwrap_or(0))
            .unwrap_or(0);
        let mut st = store.lock().expect("store lock");
        FakeServer::live(&mut st);
        match st.get(&key) {
            Some((v, _)) if v.as_slice() == owner.as_slice() => {
                drop(st);
                apply_pexpire(store, &key, ttl_ms);
                i_reply(1)
            }
            _ => i_reply(0),
        }
    } else if script.contains("'GET'") || script.contains("\"GET\"") {
        // CAS 带期望值：ARGV[0]=expected ARGV[1]=new [ARGV[2]=PX]
        let expected = args.first().cloned().unwrap_or_default();
        let new = args.get(1).cloned().unwrap_or_default();
        let px_ms: Option<u64> = args
            .get(2)
            .and_then(|b| String::from_utf8_lossy(b).parse().ok());
        let mut st = store.lock().expect("store lock");
        FakeServer::live(&mut st);
        match st.get(&key) {
            Some((v, _)) if v.as_slice() == expected.as_slice() => {
                let expires = px_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
                st.insert(key, (new, expires));
                i_reply(1)
            }
            _ => i_reply(0),
        }
    } else {
        // script_matches_known_family 已保证命中上述分支之一
        unreachable!("unknown script family reached eval_known_script")
    }
}

// ============================================================================
// 测试辅助
// ============================================================================

fn arc_bytes(b: &[u8]) -> Arc<Vec<u8>> {
    Arc::new(b.to_vec())
}

fn arc_key(k: &str) -> Arc<str> {
    Arc::from(k)
}

fn item(k: &str, v: &[u8], ttl: Option<Duration>) -> CacheSetItem {
    (arc_key(k), arc_bytes(v), ttl)
}

const DUMMY_TTL: Option<Duration> = Some(Duration::from_secs(60));

// ============================================================================
// CacheReader / CacheWriter / CacheConnector 全操作
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn reader_writer_connector_full_roundtrip() {
    let (server, backend) = connected_backend().await;

    // get miss / exists / len / is_empty / capacity
    assert_eq!(
        CacheReader::get(backend.as_ref(), "k:1").await.unwrap(),
        None
    );
    assert!(!CacheReader::exists(backend.as_ref(), "k:1").await.unwrap());
    assert!(CacheReader::is_empty(backend.as_ref()).await.unwrap());
    assert_eq!(CacheReader::capacity(backend.as_ref()).await.unwrap(), 0);

    // set with TTL → get hit / exists / ttl
    CacheWriter::set(
        backend.as_ref(),
        arc_key("k:1"),
        arc_bytes(b"v1"),
        DUMMY_TTL,
    )
    .await
    .unwrap();
    assert_eq!(
        CacheReader::get(backend.as_ref(), "k:1")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"v1"[..])
    );
    assert!(CacheReader::exists(backend.as_ref(), "k:1").await.unwrap());
    let ttl = CacheReader::ttl(backend.as_ref(), "k:1").await.unwrap();
    assert!(ttl.is_some_and(|d| d.as_secs() <= 60 && d.as_secs() >= 55));

    // set without TTL → TTL -1 → None
    CacheWriter::set(backend.as_ref(), arc_key("k:2"), arc_bytes(b"v2"), None)
        .await
        .unwrap();
    assert_eq!(
        CacheReader::ttl(backend.as_ref(), "k:2").await.unwrap(),
        None
    );

    // len / is_empty
    assert_eq!(CacheReader::len(backend.as_ref()).await.unwrap(), 2);
    assert!(!CacheReader::is_empty(backend.as_ref()).await.unwrap());

    // get_many（pipeline 路径）+ 空切片早退
    let keys = vec!["k:1".to_string(), "missing".to_string(), "k:2".to_string()];
    let many = CacheReader::get_many(backend.as_ref(), &keys)
        .await
        .unwrap();
    assert_eq!(many[0].as_deref(), Some(&b"v1"[..]));
    assert_eq!(many[1], None);
    assert_eq!(many[2].as_deref(), Some(&b"v2"[..]));
    assert!(
        CacheReader::get_many(backend.as_ref(), &[])
            .await
            .unwrap()
            .is_empty()
    );

    // keys（SCAN 循环）
    let found = CacheReader::keys(backend.as_ref(), "k:*").await.unwrap();
    assert_eq!(found.len(), 2);
    assert!(
        CacheReader::keys(backend.as_ref(), "nomatch:*")
            .await
            .unwrap()
            .is_empty()
    );

    // set_many：带 TTL 与不带 TTL 混合 + 空切片早退
    let items: Vec<CacheSetItem> = vec![item("m:1", b"m1", DUMMY_TTL), item("m:2", b"m2", None)];
    CacheWriter::set_many(backend.as_ref(), &items)
        .await
        .unwrap();
    assert_eq!(
        CacheReader::get(backend.as_ref(), "m:1")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"m1"[..])
    );
    let empty: Vec<CacheSetItem> = Vec::new();
    CacheWriter::set_many(backend.as_ref(), &empty)
        .await
        .unwrap();

    // expire：存在 true / 不存在 false
    assert!(
        CacheWriter::expire(backend.as_ref(), "m:1", DUMMY_TTL.unwrap())
            .await
            .unwrap()
    );
    assert!(
        !CacheWriter::expire(backend.as_ref(), "nope", DUMMY_TTL.unwrap())
            .await
            .unwrap()
    );

    // delete / delete_many
    CacheWriter::delete(backend.as_ref(), "m:1").await.unwrap();
    CacheWriter::delete_many(backend.as_ref(), &["m:2".to_string(), "nope".to_string()])
        .await
        .unwrap();
    assert_eq!(CacheReader::len(backend.as_ref()).await.unwrap(), 2);

    // health_check（PING）与类型识别
    CacheConnector::health_check(backend.as_ref())
        .await
        .unwrap();
    CacheConnector::shutdown(backend.as_ref()).await;
    assert_eq!(
        CacheConnector::backend_kind(backend.as_ref()),
        BackendKind::Redis
    );
    assert_eq!(BackendScore::score(backend.as_ref()), Scores::REDIS);
    assert!(BackendScore::is_persistent(backend.as_ref()));
    assert_eq!(BackendScore::backend_name(backend.as_ref()), "redis");
    assert!(CacheConnector::as_atomic_writer(backend.as_ref()).is_some());

    assert!(server.contains_key("k:1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn ttl_validation_rejects_zero_and_overflows() {
    let (_server, backend) = connected_backend().await;

    let err = CacheWriter::set(
        backend.as_ref(),
        arc_key("k"),
        arc_bytes(b"v"),
        Some(Duration::ZERO),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("at least 1 millisecond"));

    let huge = Duration::from_millis((i32::MAX as u64) * 1000 + 1);
    let err = CacheWriter::set(backend.as_ref(), arc_key("k"), arc_bytes(b"v"), Some(huge))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("exceeds Redis maximum"));

    let err = CacheWriter::expire(backend.as_ref(), "k", Duration::ZERO)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("at least 1 millisecond"));
}

#[tokio::test(flavor = "multi_thread")]
async fn stats_parses_info_sections() {
    let (_server, backend) = connected_backend().await;
    let stats = CacheReader::stats(backend.as_ref()).await.unwrap();
    assert_eq!(stats.get("type").map(String::as_str), Some("redis"));
    assert!(
        stats
            .get("memory_info")
            .is_some_and(|s| s.contains("used_memory"))
    );
    assert_eq!(
        stats.get("connected_clients").map(String::as_str),
        Some("7")
    );
    assert_eq!(stats.get("maxclients").map(String::as_str), Some("10000"));
}

#[tokio::test(flavor = "multi_thread")]
async fn atomic_writer_incr_variants() {
    let (_server, backend) = connected_backend().await;

    // delta == 1 → INCR
    assert_eq!(
        AtomicCacheWriter::incr(backend.as_ref(), "ctr", 1, None)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        AtomicCacheWriter::incr(backend.as_ref(), "ctr", 1, None)
            .await
            .unwrap(),
        2
    );

    // delta != 1 → INCRBY
    assert_eq!(
        AtomicCacheWriter::incr(backend.as_ref(), "ctr", 7, None)
            .await
            .unwrap(),
        9
    );

    // 带 TTL → EVAL（INCRBY + PEXPIRE Lua 脚本）
    assert_eq!(
        AtomicCacheWriter::incr(backend.as_ref(), "ctr:t", 5, DUMMY_TTL)
            .await
            .unwrap(),
        5
    );
    assert_eq!(
        AtomicCacheWriter::incr(backend.as_ref(), "ctr:t", 2, DUMMY_TTL)
            .await
            .unwrap(),
        7
    );

    // TTL 校验先行失败
    let err = AtomicCacheWriter::incr(backend.as_ref(), "ctr", 1, Some(Duration::ZERO))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("at least 1 millisecond"));
}

#[tokio::test(flavor = "multi_thread")]
async fn compare_and_swap_all_script_variants() {
    let (_server, backend) = connected_backend().await;

    // 无期望值 + 无 TTL：不存在 → 成功
    assert!(
        AtomicCacheWriter::compare_and_swap(backend.as_ref(), "cas:a", None, b"n1".to_vec(), None)
            .await
            .unwrap()
    );
    // 已存在 → 失败
    assert!(
        !AtomicCacheWriter::compare_and_swap(backend.as_ref(), "cas:a", None, b"n2".to_vec(), None)
            .await
            .unwrap()
    );
    // 无期望值 + TTL
    assert!(
        AtomicCacheWriter::compare_and_swap(
            backend.as_ref(),
            "cas:b",
            None,
            b"n1".to_vec(),
            DUMMY_TTL
        )
        .await
        .unwrap()
    );
    // 带期望值 + 无 TTL：期望匹配 → 成功
    assert!(
        AtomicCacheWriter::compare_and_swap(
            backend.as_ref(),
            "cas:a",
            Some(b"n1".as_slice()),
            b"n2".to_vec(),
            None
        )
        .await
        .unwrap()
    );
    // 期望不匹配 → 失败
    assert!(
        !AtomicCacheWriter::compare_and_swap(
            backend.as_ref(),
            "cas:a",
            Some(b"zzz".as_slice()),
            b"n3".to_vec(),
            None
        )
        .await
        .unwrap()
    );
    // 带期望值 + TTL：匹配 → 成功并写入
    assert!(
        AtomicCacheWriter::compare_and_swap(
            backend.as_ref(),
            "cas:a",
            Some(b"n2".as_slice()),
            b"n3".to_vec(),
            DUMMY_TTL
        )
        .await
        .unwrap()
    );
    assert_eq!(
        CacheReader::get(backend.as_ref(), "cas:a")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"n3"[..])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn set_if_absent_nx_semantics() {
    let (_server, backend) = connected_backend().await;

    assert!(
        AtomicCacheWriter::set_if_absent(backend.as_ref(), "nx:1", b"v".to_vec(), None)
            .await
            .unwrap()
    );
    assert!(
        !AtomicCacheWriter::set_if_absent(backend.as_ref(), "nx:1", b"w".to_vec(), None)
            .await
            .unwrap()
    );
    assert!(
        AtomicCacheWriter::set_if_absent(backend.as_ref(), "nx:2", b"v".to_vec(), DUMMY_TTL)
            .await
            .unwrap()
    );
    assert_eq!(
        CacheReader::get(backend.as_ref(), "nx:2")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"v"[..])
    );
}

// ============================================================================
// pipeline 模块（pub 批量方法）
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn pipeline_batch_methods() {
    let (_server, backend) = connected_backend().await;

    // 空切片早退
    backend.set_many_pipeline(&[], None).await.unwrap();
    assert!(backend.get_many_pipeline(&[]).await.unwrap().is_empty());
    backend.delete_many_pipeline(&[]).await.unwrap();

    let items = vec![("p:1", b"a".to_vec()), ("p:2", b"b".to_vec())];
    backend.set_many_pipeline(&items, DUMMY_TTL).await.unwrap();
    let got = backend
        .get_many_pipeline(&["p:1", "p:missing", "p:2"])
        .await
        .unwrap();
    assert_eq!(got[0].as_deref(), Some(&b"a"[..]));
    assert_eq!(got[1], None);
    assert_eq!(got[2].as_deref(), Some(&b"b"[..]));

    // TTL 校验失败路径
    let err = backend
        .set_many_pipeline(&items, Some(Duration::ZERO))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("at least 1 millisecond"));

    backend.delete_many_pipeline(&["p:1", "p:2"]).await.unwrap();
    assert!(backend.get_many_pipeline(&["p:1", "p:2"]).await.unwrap()[0].is_none());

    // 无 TTL 的批量写
    backend.set_many_pipeline(&items, None).await.unwrap();
    assert_eq!(
        backend.get_many_pipeline(&["p:1"]).await.unwrap()[0].as_deref(),
        Some(&b"a"[..])
    );
}

// ============================================================================
// namespace（SCAN + 批量 DEL）与 dangerous clear
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn clear_namespace_and_dangerous_clear() {
    let (server, backend) = connected_backend().await;

    // 空前缀 / 通配符 / glob 字符类拒绝
    let err = backend.clear_namespace("").await.unwrap_err();
    assert!(err.to_string().contains("must not be empty"));
    let err = backend.clear_namespace("ns:*").await.unwrap_err();
    assert!(err.to_string().contains("wildcard"));
    let err = backend.clear_namespace("ns:[a]").await.unwrap_err();
    assert!(err.to_string().contains("glob pattern"));

    // 危险 clear 默认关闭
    let err = CacheWriter::clear(backend.as_ref()).await.unwrap_err();
    assert!(err.to_string().contains("disabled by default"));

    // clear_namespace 仅删前缀命中键
    for k in ["ns:a", "ns:b", "other:c"] {
        CacheWriter::set(backend.as_ref(), arc_key(k), arc_bytes(b"x"), None)
            .await
            .unwrap();
    }
    backend.clear_namespace("ns:").await.unwrap();
    assert!(!server.contains_key("ns:a"));
    assert!(!server.contains_key("ns:b"));
    assert!(server.contains_key("other:c"));

    // dangerous_clear_enabled(true) → SCAN + 批量 DEL 清库
    let (server2, url2) = FakeServer::spawn();
    let backend2 = RedisBackend::builder()
        .connection_string(&url2)
        .retry_count(0)
        .dangerous_clear_enabled(true)
        .build()
        .await
        .unwrap();
    CacheWriter::set(&backend2, arc_key("d:1"), arc_bytes(b"x"), None)
        .await
        .unwrap();
    CacheWriter::set(&backend2, arc_key("d:2"), arc_bytes(b"x"), None)
        .await
        .unwrap();
    CacheWriter::clear(&backend2).await.unwrap();
    assert!(!server2.contains_key("d:1") && !server2.contains_key("d:2"));
}

// ============================================================================
// LuaExecutor（eval_lua / eval_sha / script_load）
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn lua_executor_paths() {
    let (_server, backend) = connected_backend().await;

    // eval_lua 成功路径（假服务器对未知脚本回 :1）
    let value = backend
        .eval_lua("return 1", &["lua:k"], &["1"])
        .await
        .unwrap();
    assert!(matches!(value, oxcache::redis::Value::Int(1)));

    // eval_lua 黑名单脚本拒绝
    let err = backend
        .eval_lua("return redis.call('FLUSHALL')", &[], &[])
        .await
        .unwrap_err();
    assert!(err.to_string().contains("FLUSHALL"));

    // eval_lua 非法键名拒绝（含空白字符的键）
    let err = backend
        .eval_lua("return 1", &["bad key\n"], &[])
        .await
        .unwrap_err();
    assert!(!err.to_string().is_empty());

    // eval_sha：SHA 格式校验（客户端侧）
    let err = backend.eval_sha("short", &[], &[]).await.unwrap_err();
    assert!(err.to_string().contains("Invalid SHA format"));
    let err = backend
        .eval_sha(&"g".repeat(40), &[], &[])
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Invalid SHA format"));

    // eval_sha：未加载脚本 → NOSCRIPT 显性错误
    let err = backend
        .eval_sha(&"a".repeat(40), &["lua:k"], &[])
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("noscript"),
        "NOSCRIPT 错误必须透传，实际: {err}"
    );

    // script_load 后 eval_sha 成功
    let sha = backend.script_load("return 1").await.unwrap();
    assert_eq!(sha.len(), 40);
    let value = backend.eval_sha(&sha, &["lua:k"], &[]).await.unwrap();
    assert!(matches!(value, oxcache::redis::Value::Int(1)));
}

// ============================================================================
// client 层：ping / with_pool / 断路器
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn client_ping_and_with_pool() {
    let (_server, url) = FakeServer::spawn();
    let backend = RedisBackend::with_pool(&url, 4).await.unwrap();
    assert_eq!(backend.ping().await.unwrap(), "PONG");
    assert_eq!(
        backend.mode(),
        oxcache::backend::memory::RedisMode::Standalone
    );
    let _ = backend.client();
}

#[tokio::test(flavor = "multi_thread")]
async fn circuit_breaker_opens_after_failures() {
    let (server, url) = FakeServer::spawn();
    let backend = RedisBackend::builder()
        .connection_string(&url)
        .retry_count(0)
        .circuit_breaker_threshold(1)
        .circuit_breaker_reset_timeout(Duration::from_millis(500))
        .build()
        .await
        .unwrap();

    server.set_fail_all(true);
    // 第一次失败：普通错误（重试 0 次）
    let err = CacheReader::get(&backend, "any").await.unwrap_err();
    assert!(!err.to_string().contains("circuit breaker"));
    // 第二次：断路器已开 → Degraded
    let err = CacheReader::get(&backend, "any").await.unwrap_err();
    assert!(err.to_string().contains("circuit breaker is open"));

    server.set_fail_all(false);
    // 复位超时前仍 Degraded
    let err = CacheReader::get(&backend, "any").await.unwrap_err();
    assert!(err.to_string().contains("circuit breaker is open"));
}

// ============================================================================
// 同步面（block_in_place 桥接）
// ============================================================================

#[test]
fn sync_surface_full_roundtrip() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let (_server, backend) = connected_backend().await;
        use oxcache::backend::{
            SyncAtomicCacheWriter, SyncCacheConnector, SyncCacheReader, SyncCacheWriter,
        };

        // 先经 async 面预热连接
        CacheWriter::set(backend.as_ref(), arc_key("s:1"), arc_bytes(b"v"), None)
            .await
            .unwrap();

        assert_eq!(
            SyncCacheReader::get(backend.as_ref(), "s:1")
                .unwrap()
                .as_deref(),
            Some(&b"v"[..])
        );
        assert!(SyncCacheReader::exists(backend.as_ref(), "s:1").unwrap());
        assert_eq!(SyncCacheReader::ttl(backend.as_ref(), "s:1").unwrap(), None);
        assert_eq!(SyncCacheReader::len(backend.as_ref()).unwrap(), 1);
        assert!(!SyncCacheReader::is_empty(backend.as_ref()).unwrap());
        assert_eq!(SyncCacheReader::capacity(backend.as_ref()).unwrap(), 0);
        let stats = SyncCacheReader::stats(backend.as_ref()).unwrap();
        assert_eq!(stats.get("type").map(String::as_str), Some("redis"));
        // 同步面 keys 未被 RedisBackend 覆写 → 走 trait 默认实现（空）
        let keys = SyncCacheReader::keys(backend.as_ref(), "s:*").unwrap();
        assert!(keys.is_empty());
        let many =
            SyncCacheReader::get_many(backend.as_ref(), &["s:1".to_string(), "x".to_string()])
                .unwrap();
        assert_eq!(many[0].as_deref(), Some(&b"v"[..]));
        assert_eq!(many[1], None);

        SyncCacheWriter::set(backend.as_ref(), arc_key("s:2"), arc_bytes(b"w"), DUMMY_TTL).unwrap();
        assert!(SyncCacheWriter::expire(backend.as_ref(), "s:2", Duration::from_secs(30)).unwrap());
        SyncCacheWriter::delete(backend.as_ref(), "s:2").unwrap();
        SyncCacheWriter::set_many(backend.as_ref(), &[item("s:3", b"z", None)]).unwrap();
        SyncCacheWriter::delete_many(backend.as_ref(), &["s:3".to_string()]).unwrap();
        // 危险 clear 默认拒绝（同步面同样受安全门控约束）
        let err = SyncCacheWriter::clear(backend.as_ref()).unwrap_err();
        assert!(err.to_string().contains("disabled by default"));
        assert_eq!(SyncCacheReader::len(backend.as_ref()).unwrap(), 1);

        SyncCacheConnector::health_check(backend.as_ref()).unwrap();
        SyncCacheConnector::shutdown(backend.as_ref());
        assert_eq!(
            SyncCacheConnector::backend_kind(backend.as_ref()),
            BackendKind::Redis
        );
        assert!(SyncCacheConnector::as_sync_atomic_writer(backend.as_ref()).is_some());

        assert_eq!(
            SyncAtomicCacheWriter::incr(backend.as_ref(), "s:ctr", 3, None).unwrap(),
            3
        );
        assert!(
            SyncAtomicCacheWriter::set_if_absent(backend.as_ref(), "s:nx", b"v".to_vec(), None)
                .unwrap()
        );
        assert!(
            !SyncAtomicCacheWriter::compare_and_swap(
                backend.as_ref(),
                "s:nx",
                Some(b"nope".as_slice()),
                b"v2".to_vec(),
                None
            )
            .unwrap()
        );
    });
}

// ============================================================================
// Dragonfly 后端（RedisBackend 委托层）
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn dragonfly_backend_delegates_full_surface() {
    use oxcache::backend::DragonflyBackend;
    use oxcache::backend::dragonfly::DragonflyRestrictions;

    let (_server, url) = FakeServer::spawn();
    let backend = DragonflyBackend::new(&url, 2).await.unwrap();

    // 约束集元数据
    let restrictions = backend.restrictions();
    assert!(restrictions.is_command_disabled("FLUSHALL"));
    assert!(!restrictions.is_command_disabled("GET"));
    assert!(restrictions.cluster_disabled());
    let custom = DragonflyRestrictions::default()
        .with_disabled_commands(vec!["DEBUG".to_string()])
        .with_cluster_disabled(false);
    assert!(custom.is_command_disabled("DEBUG"));
    assert!(!custom.cluster_disabled());

    let backend = backend.with_restrictions(custom);
    assert!(backend.restrictions().is_command_disabled("DEBUG"));
    let _ = backend.inner();

    // 委托全操作
    assert_eq!(CacheReader::get(&backend, "d:k").await.unwrap(), None);
    CacheWriter::set(&backend, arc_key("d:k"), arc_bytes(b"dv"), DUMMY_TTL)
        .await
        .unwrap();
    assert_eq!(
        CacheReader::get(&backend, "d:k").await.unwrap().as_deref(),
        Some(&b"dv"[..])
    );
    assert!(CacheReader::exists(&backend, "d:k").await.unwrap());
    assert!(CacheReader::ttl(&backend, "d:k").await.unwrap().is_some());
    assert_eq!(CacheReader::len(&backend).await.unwrap(), 1);
    assert_eq!(CacheReader::capacity(&backend).await.unwrap(), 0);
    let stats = CacheReader::stats(&backend).await.unwrap();
    assert!(stats.contains_key("memory_info"));
    let keys = CacheReader::keys(&backend, "d:*").await.unwrap();
    assert_eq!(keys.len(), 1);

    CacheWriter::expire(&backend, "d:k", DUMMY_TTL.unwrap())
        .await
        .unwrap();
    let items: Vec<CacheSetItem> = vec![item("d:m", b"mv", None)];
    CacheWriter::set_many(&backend, &items).await.unwrap();
    CacheWriter::delete_many(&backend, &["d:m".to_string()])
        .await
        .unwrap();
    CacheWriter::delete(&backend, "d:k").await.unwrap();

    CacheConnector::health_check(&backend).await.unwrap();
    CacheConnector::shutdown(&backend).await;
    assert_eq!(
        CacheConnector::backend_kind(&backend),
        BackendKind::Dragonfly
    );
    // Dragonfly 原子操作兼容性未验证 → 探针如实返回 None
    assert!(CacheConnector::as_atomic_writer(&backend).is_none());
    assert_eq!(BackendScore::score(&backend), Scores::REDIS);
    assert!(BackendScore::is_persistent(&backend));
    assert_eq!(BackendScore::backend_name(&backend), "dragonfly");
}

// ============================================================================
// 分布式锁：生命周期 / 争用 / 偷锁 / provider 抽象 / watchdog
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn dist_lock_full_lifecycle() {
    let (_server, backend) = connected_backend().await;

    let mut lock = DistLockBuilder::new(Arc::clone(&backend), "lock:life".to_string())
        .ttl(Duration::from_secs(30))
        .watchdog_enabled(false)
        .build();
    assert_eq!(lock.token(), 0, "未获取时 fencing token 为 0");

    // 首次获取
    assert!(lock.acquire().await.unwrap());
    assert!(lock.token() > 0, "获取成功后 fencing token 单调递增");
    assert!(lock.is_held().await.unwrap());
    assert!(lock.extend().await.unwrap());

    // 重入获取 → Ok(false)
    assert!(!lock.acquire().await.unwrap());

    // 第二持有者争用 → Err
    let mut rival = DistLockBuilder::new(Arc::clone(&backend), "lock:life".to_string())
        .ttl(Duration::from_secs(30))
        .watchdog_enabled(false)
        .build();
    let err = rival.acquire().await.unwrap_err();
    assert!(err.to_string().contains("already held by another owner"));

    // 第一次 release：重入计数 2→1，仅本地递减，不触碰远程
    lock.release().await.unwrap();
    assert!(lock.is_held().await.unwrap());
    // 第二次 release：计数 1→0，执行远程 Lua 释放
    lock.release().await.unwrap();
    assert!(!lock.is_held().await.unwrap());
    assert!(!lock.extend().await.unwrap());

    // 未持锁释放 → Err
    let err = lock.release().await.unwrap_err();
    assert!(err.to_string().contains("not held"));

    // 偷锁后释放 → 显性失败
    let mut victim = DistLockBuilder::new(Arc::clone(&backend), "lock:steal".to_string())
        .ttl(Duration::from_secs(30))
        .watchdog_enabled(false)
        .build();
    assert!(victim.acquire().await.unwrap());
    CacheWriter::delete(backend.as_ref(), "lock:steal")
        .await
        .unwrap();
    let err = victim.release().await.unwrap_err();
    assert!(err.to_string().contains("not held by owner"));
}

#[tokio::test(flavor = "multi_thread")]
async fn lock_provider_trait_surface() {
    let (_server, backend) = connected_backend().await;

    let mut lock = DistLockBuilder::new(Arc::clone(&backend), "lock:prov".to_string())
        .ttl(Duration::from_secs(30))
        .watchdog_enabled(false)
        .build();

    let provider: &mut dyn LockProvider = &mut lock;
    // try_lock 单次获取
    assert!(provider.try_lock().await.unwrap());
    // 已持有（重入）→ Ok(false)，走争用错误翻译分支
    assert!(!provider.try_lock().await.unwrap());
    // is_held 经 trait 转发
    assert!(provider.is_held().await.unwrap());
    // fencing_token 有值
    assert!(provider.fencing_token().is_some());
    // unlock 经 trait 转发：第一次仅本地递减（重入计数 2→1），第二次远程释放
    provider.unlock().await.unwrap();
    assert!(provider.is_held().await.unwrap());
    provider.unlock().await.unwrap();
    assert!(!provider.is_held().await.unwrap());

    // 未持有 → fencing_token None
    let (_s2, url2) = FakeServer::spawn();
    let fresh = DistLockBuilder::new(Arc::new(backend_on(&url2).await), "lock:prov2".to_string())
        .watchdog_enabled(false)
        .build();
    let fresh_token = {
        let p: &dyn LockProvider = &fresh;
        p.fencing_token()
    };
    assert_eq!(fresh_token, None);

    // lock() 指数退避重试直到接管过期锁
    let (_s3, url3) = FakeServer::spawn();
    let b3 = Arc::new(backend_on(&url3).await);
    let mut holder = DistLockBuilder::new(Arc::clone(&b3), "lock:prov3".to_string())
        .ttl(Duration::from_millis(200))
        .watchdog_enabled(false)
        .build();
    assert!(holder.acquire().await.unwrap());
    let contender = DistLockBuilder::new(Arc::clone(&b3), "lock:prov3".to_string())
        .ttl(Duration::from_millis(200))
        .watchdog_enabled(false)
        .build();
    let waiter = tokio::spawn(async move {
        let mut contender: Box<dyn LockProvider> = Box::new(contender);
        let acquired = contender.lock().await;
        (contender, acquired)
    });
    // 退避序列 50→100→200ms 内锁过期，竞争者接管
    let (contender, acquired) = waiter.await.unwrap();
    assert!(acquired.is_ok(), "lock() 应在锁过期后接管");
    assert!(contender.is_held().await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn watchdog_renews_and_backs_off_on_errors() {
    let (server, backend) = connected_backend().await;

    let key = "lock:watchdog".to_string();
    let mut lock = DistLockBuilder::new(backend, key)
        .ttl(Duration::from_millis(3000))
        .watchdog_enabled(true)
        .build();
    assert!(lock.acquire().await.unwrap());

    // 续期周期 ≈ ttl/3 = 1s；t≈1s 已续期（存活至 ≈4s），锁必须仍在手
    tokio::time::sleep(Duration::from_millis(1400)).await;
    assert!(lock.is_held().await.unwrap(), "watchdog 必须在 TTL 内续期");

    // 注入 Redis 错误：t≈2s 的续期尝试失败 → watchdog 进入指数退避而不弃锁
    //（错误窗口内不得调用任何 API）
    server.set_fail_all(true);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    server.set_fail_all(false);
    // 退避（200ms→400ms）恢复后 t≈2.6s 续期成功
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(
        lock.is_held().await.unwrap(),
        "错误退避后 watchdog 恢复续期"
    );

    // 释放终止 watchdog
    lock.release().await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!lock.is_held().await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn watchdog_exits_when_lock_stolen() {
    let (server, backend) = connected_backend().await;

    let mut lock = DistLockBuilder::new(backend, "lock:stolen-wd".to_string())
        .ttl(Duration::from_millis(600))
        .watchdog_enabled(true)
        .build();
    assert!(lock.acquire().await.unwrap());

    // 模拟锁被偷：等一次续期后直接删键
    tokio::time::sleep(Duration::from_millis(400)).await;
    server.remove_key("lock:stolen-wd");
    // 下一次续期 Ok(0) → watchdog 退出；此时锁必然不在手
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert!(
        !lock.is_held().await.unwrap(),
        "锁被偷后 watchdog 退出且锁不在手"
    );
}

// ============================================================================
// builder 细节
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn builder_validation_and_database_suffix() {
    let (_server, url) = FakeServer::spawn();

    // pool_size = 0 拒绝
    let refused = RedisBackendBuilder::default()
        .connection_string(&url)
        .pool_size(0)
        .build()
        .await;
    let err = match refused {
        Err(e) => e,
        Ok(_) => panic!("pool_size=0 必须被拒绝"),
    };
    assert!(err.to_string().contains("pool size"));

    // database 附加 /N 后缀（假服务器 SELECT → +OK）
    let backend = RedisBackendBuilder::default()
        .connection_string(&url)
        .retry_count(0)
        .database(2)
        .build()
        .await
        .unwrap();
    assert_eq!(backend.ping().await.unwrap(), "PONG");

    // Valkey 模式 → backend_kind Valkey
    let valkey = RedisBackendBuilder::default()
        .connection_string(&url)
        .retry_count(0)
        .mode(oxcache::backend::memory::RedisMode::ValkeyStandalone)
        .build()
        .await
        .unwrap();
    assert_eq!(CacheConnector::backend_kind(&valkey), BackendKind::Valkey);

    // 连接不可达 → Connection 错误（127.0.0.1 未监听端口）
    let dead = RedisBackendBuilder::default()
        .connection_string("redis://127.0.0.1:1")
        .connection_timeout(Duration::from_millis(300))
        .build()
        .await;
    assert!(dead.is_err());
}

// ============================================================================
// Cache::redis 构造器（api_impl）：经进程内假服务器走完整构建
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn cache_redis_constructor_full_path() {
    let (_server, url) = FakeServer::spawn();
    let cache: oxcache::Cache<String, String> = oxcache::Cache::redis(&url).await.unwrap();
    cache
        .set(&"rc".to_string(), &"ok".to_string())
        .await
        .unwrap();
    assert_eq!(
        cache.get(&"rc".to_string()).await.unwrap().as_deref(),
        Some("ok")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_redis_constructor_unreachable_fails() {
    let result: OxCacheResult<oxcache::Cache<String, String>> =
        oxcache::Cache::redis("redis://127.0.0.1:1").await;
    assert!(result.is_err());
}
