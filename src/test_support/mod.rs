// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 测试内嵌最小 RESP2 假 Redis 服务器（仅 `#[cfg(test)]` 编译）。
//!
//! 供需要"真实 Redis 端点"的内联测试（dragonfly / dist_lock 等）在无外部
//! 服务的环境下运行：测试先调 [`ensure_server`]，端口空闲则拉起本进程内的
//! 假服务器，端口已被占用则探测 PING 判定可用性。只实现本项目实际下发的
//! 命令语义；EVAL 按已知脚本关键字分发到等价原生语义（与
//! `tests/redis_fake_backend_test.rs` 同一套语义，二者独立编译互不依赖）。

#![cfg(test)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Store = HashMap<String, (Vec<u8>, Option<Instant>)>;

/// 确保目标端口有可用的 Redis 兼容端点。
///
/// - 端口空闲：拉起进程内假服务器并返回 true。
/// - 端口被占用：PING 探测，可用（真实服务）返回 true，否则 false。
pub(crate) fn ensure_server(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => {
            spawn_on_listener(listener);
            true
        }
        Err(_) => ping_port(port),
    }
}

fn ping_port(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream.write_all(b"*1\r\n$4\r\nPING\r\n").is_err() {
        return false;
    }
    let mut reply = [0u8; 7];
    match stream.read(&mut reply) {
        Ok(n) => n > 0 && reply.starts_with(b"+PONG"),
        Err(_) => false,
    }
}

fn spawn_on_listener(listener: TcpListener) {
    let _ = listener.set_nonblocking(true);
    let store: Arc<Mutex<Store>> = Arc::new(Mutex::new(HashMap::new()));
    let scripts: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    std::thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let store = Arc::clone(&store);
                    let scripts = Arc::clone(&scripts);
                    std::thread::spawn(move || serve_connection(stream, store, scripts));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(_) => break,
            }
        }
    });
}

fn serve_connection(
    stream: TcpStream,
    store: Arc<Mutex<Store>>,
    scripts: Arc<Mutex<HashMap<String, String>>>,
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
        let mut buf: Vec<u8> = Vec::new();
        for reply in execute(&argv, &store, &scripts) {
            buf.extend_from_slice(&reply);
        }
        if out.write_all(&buf).is_err() {
            break;
        }
        let _ = out.flush();
    }
}

fn read_command(reader: &mut impl BufRead) -> std::io::Result<Option<Vec<Vec<u8>>>> {
    let mut header = Vec::new();
    if reader.read_until(b'\n', &mut header)? == 0 {
        return Ok(None);
    }
    let header = String::from_utf8_lossy(&header);
    let header = header.trim_end();
    let Some(stripped) = header.strip_prefix('*') else {
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
        let mut body = vec![0u8; len + 2];
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

fn live(store: &mut Store) -> &mut Store {
    let now = Instant::now();
    store.retain(|_, (_, expires)| expires.is_none_or(|at| at > now));
    store
}

fn glob_match(pattern: &str, text: &str) -> bool {
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

fn execute(
    argv: &[Vec<u8>],
    store: &Arc<Mutex<Store>>,
    _scripts: &Arc<Mutex<HashMap<String, String>>>,
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
        "AUTH" | "SELECT" | "CLIENT" => vec![s_reply("OK")],
        "FLUSHALL" | "FLUSHDB" => {
            store.lock().expect("store lock").clear();
            vec![s_reply("OK")]
        }
        "DBSIZE" => {
            let mut st = store.lock().expect("store lock");
            live(&mut st);
            vec![i_reply(st.len() as i64)]
        }
        "GET" => {
            let mut st = store.lock().expect("store lock");
            live(&mut st);
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
            live(&mut st);
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
            live(&mut st);
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
            live(&mut st);
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
            live(&mut st);
            let key = arg(0);
            vec![i_reply(i64::from(st.contains_key(&key)))]
        }
        "PEXPIRE" | "EXPIRE" => {
            let key = arg(0);
            let ttl: u64 = arg(1).parse().unwrap_or(0);
            let ms = if cmd == "PEXPIRE" { ttl } else { ttl * 1000 };
            let mut st = store.lock().expect("store lock");
            live(&mut st);
            match st.get_mut(&key) {
                Some((_, expires)) => {
                    *expires = Some(Instant::now() + Duration::from_millis(ms));
                    vec![i_reply(1)]
                }
                None => vec![i_reply(0)],
            }
        }
        "TTL" => {
            let mut st = store.lock().expect("store lock");
            live(&mut st);
            let key = arg(0);
            match st.get(&key) {
                None => vec![i_reply(-2)],
                Some((_, None)) => vec![i_reply(-1)],
                Some((_, Some(at))) => {
                    let ms = at.saturating_duration_since(Instant::now()).as_millis() as i64;
                    vec![i_reply((ms + 999) / 1000)]
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
            live(&mut st);
            let keys: Vec<String> = st
                .keys()
                .filter(|k| glob_match(&pattern, k))
                .cloned()
                .collect();
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
            live(&mut st);
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
        "EVAL" => vec![eval_known_script(store, argv)],
        _ => vec![err_reply(&format!("unknown command '{cmd}'"))],
    }
}

fn incr_by(store: &Arc<Mutex<Store>>, key: &str, delta: i64) -> Vec<u8> {
    let mut st = store.lock().expect("store lock");
    live(&mut st);
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

fn eval_known_script(store: &Arc<Mutex<Store>>, argv: &[Vec<u8>]) -> Vec<u8> {
    let script = String::from_utf8_lossy(&argv[1]).into_owned();
    if !script_matches_known_family(&script) {
        // 未知脚本（测试自造）：等价 return 1
        return i_reply(1);
    }
    let key = argv
        .get(3)
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    let args: Vec<Vec<u8>> = argv.iter().skip(4).cloned().collect();

    if script.contains("INCRBY") {
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
        live(&mut st);
        match st.get(&key) {
            Some((v, _)) if v.as_slice() == owner.as_slice() => {
                st.remove(&key);
                i_reply(1)
            }
            _ => i_reply(0),
        }
    } else if script.contains("'EXISTS'") || script.contains("\"EXISTS\"") {
        // CAS 无期望值（set-if-absent）
        let new = args.first().cloned().unwrap_or_default();
        let px_ms: Option<u64> = args
            .get(1)
            .and_then(|b| String::from_utf8_lossy(b).parse().ok());
        let mut st = store.lock().expect("store lock");
        live(&mut st);
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
        live(&mut st);
        match st.get(&key) {
            Some((v, _)) if v.as_slice() == owner.as_slice() => {
                drop(st);
                apply_pexpire(store, &key, ttl_ms);
                i_reply(1)
            }
            _ => i_reply(0),
        }
    } else {
        // CAS 带期望值：ARGV[0]=expected ARGV[1]=new [ARGV[2]=PX]
        let expected = args.first().cloned().unwrap_or_default();
        let new = args.get(1).cloned().unwrap_or_default();
        let px_ms: Option<u64> = args
            .get(2)
            .and_then(|b| String::from_utf8_lossy(b).parse().ok());
        let mut st = store.lock().expect("store lock");
        live(&mut st);
        match st.get(&key) {
            Some((v, _)) if v.as_slice() == expected.as_slice() => {
                let expires = px_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
                st.insert(key, (new, expires));
                i_reply(1)
            }
            _ => i_reply(0),
        }
    }
}

fn fake_sha1(script: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in script {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}").repeat(5)[..40].to_string()
}

#[cfg(test)]
mod tests;
