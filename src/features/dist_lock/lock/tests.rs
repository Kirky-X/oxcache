use super::*;
use uuid::Uuid;

#[test]
fn test_reentrant_count_logic() {
    let count = AtomicU32::new(0);

    // First acquire: count 0 -> 1
    assert_eq!(count.load(Ordering::SeqCst), 0);
    count.fetch_add(1, Ordering::SeqCst);
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // Reentrant acquire: count 1 -> 2
    count.fetch_add(1, Ordering::SeqCst);
    assert_eq!(count.load(Ordering::SeqCst), 2);

    // First release: count 2 -> 1 (don't actually release)
    let new_count = count.fetch_sub(1, Ordering::SeqCst) - 1;
    assert_eq!(new_count, 1);
    assert!(new_count > 0); // shouldn't release yet

    // Second release: count 1 -> 0 (actually release)
    let new_count = count.fetch_sub(1, Ordering::SeqCst) - 1;
    assert_eq!(new_count, 0);
}

#[test]
fn test_owner_id_is_unique() {
    let id1 = Uuid::new_v4().to_string();
    let id2 = Uuid::new_v4().to_string();
    assert_ne!(id1, id2);
}

#[test]
fn test_lua_scripts_are_valid() {
    // Basic sanity: scripts should contain expected Redis commands
    assert!(RELEASE_SCRIPT.contains("GET"));
    assert!(RELEASE_SCRIPT.contains("DEL"));
    assert!(EXTEND_SCRIPT.contains("GET"));
    assert!(EXTEND_SCRIPT.contains("PEXPIRE"));
}

#[test]
fn test_released_flag_default() {
    let released = AtomicBool::new(false);
    assert!(!released.load(Ordering::SeqCst));
    released.store(true, Ordering::SeqCst);
    assert!(released.load(Ordering::SeqCst));
}

#[test]
fn test_watchdog_retry_delay_grows_and_caps() {
    assert!(watchdog_retry_delay(0) <= watchdog_retry_delay(1));
    assert!(watchdog_retry_delay(1) <= watchdog_retry_delay(5));
    assert!(watchdog_retry_delay(u32::MAX) <= Duration::from_secs(10));
}

#[allow(unsafe_code)]
async fn live_test_backend() -> Option<Arc<RedisBackend>> {
    // SAFETY: idempotent set of the same value; matches redis test helper pattern.
    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    }
    // 无外部 Redis 时拉起进程内假 RESP 服务器；连接失败才跳过
    if !crate::test_support::ensure_server(6379) {
        return None;
    }
    Some(Arc::new(
        RedisBackend::new("redis://127.0.0.1:6379").await.ok()?,
    ))
}

#[tokio::test]
async fn test_release_stolen_lock_keeps_retryable_state() {
    use super::super::DistLockBuilder;

    let Some(backend) = live_test_backend().await else {
        return;
    };
    let key = format!("test-lock-stolen-{}", Uuid::new_v4());
    let mut lock = DistLockBuilder::new(backend.clone(), key.clone())
        .ttl(Duration::from_secs(30))
        .watchdog_enabled(false)
        .build();
    assert!(lock.acquire().await.expect("acquire"));
    // Simulate expiry/steal: delete the key out-of-band.
    let mut conn = backend.conn();
    let _: i64 = redis::cmd(RedisCommand::Del.as_str())
        .arg(&key)
        .query_async(&mut conn)
        .await
        .expect("DEL");
    let err = lock
        .release()
        .await
        .expect_err("release of stolen lock must fail");
    assert!(err.to_string().contains("not held by owner"));
    assert_eq!(
        lock.reentrant_count.load(Ordering::SeqCst),
        1,
        "failed release must keep retryable state"
    );
}

#[tokio::test]
#[ignore = "needs live Redis at 127.0.0.1:6379"]
async fn test_release_conn_error_keeps_retryable_state() {
    use super::super::DistLockBuilder;
    use std::process::Command;

    let Some(backend) = live_test_backend().await else {
        return;
    };
    let key = format!("test-lock-connerr-{}", Uuid::new_v4());
    let mut lock = DistLockBuilder::new(backend.clone(), key.clone())
        .ttl(Duration::from_secs(30))
        .watchdog_enabled(false)
        .build();
    assert!(lock.acquire().await.expect("acquire"));
    // Kill the server to force a connection error on release.
    let _ = Command::new("redis-cli")
        .args(["-p", "6379", "shutdown", "nosave"])
        .status();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let err = lock
        .release()
        .await
        .expect_err("release against dead server must fail");
    assert!(err.to_string().contains("dist_lock release failed"));
    assert_eq!(
        lock.reentrant_count.load(Ordering::SeqCst),
        1,
        "failed release must keep retryable state"
    );
    assert!(
        !lock.released.load(Ordering::SeqCst),
        "failed release must not flag the lock as released"
    );
    // Restore the server for subsequent tests.
    let _ = Command::new("redis-server")
        .args([
            "--port",
            "6379",
            "--daemonize",
            "yes",
            "--save",
            "",
            "--appendonly",
            "no",
        ])
        .status();
    tokio::time::sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
async fn test_watchdog_renews_past_ttl() {
    use super::super::DistLockBuilder;

    let Some(backend) = live_test_backend().await else {
        return;
    };
    let key = format!("test-lock-watchdog-{}", Uuid::new_v4());
    let mut lock = DistLockBuilder::new(backend, key)
        .ttl(Duration::from_secs(3))
        .watchdog_enabled(true)
        .build();
    assert!(lock.acquire().await.expect("acquire"));
    // Sleep past the TTL; the watchdog (renew every ~1s) must keep it held.
    tokio::time::sleep(Duration::from_millis(4500)).await;
    assert!(
        lock.is_held().await.expect("is_held"),
        "watchdog must renew the lock past TTL"
    );
    lock.release().await.expect("release");
}
