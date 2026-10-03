// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! RedbDiskBackend 长尾面：open/create 分支、默认 TTL、懒过期物理删除、
//! stats/len/expire/exists 与 path 诊断。

#![cfg(feature = "disk")]

use std::time::Duration;

use oxcache::backend::disk::RedbDiskBackend;
use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
use std::sync::Arc;

fn temp_path(tag: &str) -> std::path::PathBuf {
    let dir = tempfile::tempdir().unwrap();
    // tempdir 在返回时删除；这里把文件放进独立 dir 并保留句柄语义——
    // 直接用系统临时目录下的唯一文件名，由 redb 负责创建
    let mut p = std::env::temp_dir();
    p.push(format!(
        "oxcache-disk-extra-{tag}-{}.redb",
        std::process::id()
    ));
    let _ = dir;
    p
}

#[tokio::test]
async fn open_existing_reports_path() {
    let path = temp_path("create");
    // create 建库后 open 打开既有文件（诊断 path 面一致）
    RedbDiskBackend::create(&path).unwrap();
    let backend = RedbDiskBackend::open(&path).expect("open existing file");
    assert_eq!(backend.path(), path.as_path());

    CacheWriter::set(
        &backend,
        Arc::from("k"),
        Arc::new(b"v".to_vec()),
        Some(Duration::from_secs(30)),
    )
    .await
    .unwrap();
    assert_eq!(
        CacheReader::get(&backend, "k").await.unwrap().as_deref(),
        Some(&b"v"[..])
    );
    CacheConnector::shutdown(&backend).await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn default_ttl_applies_to_ttl_less_writes() {
    let path = temp_path("default-ttl");
    let backend = RedbDiskBackend::create(&path)
        .unwrap()
        .with_default_ttl(Duration::from_secs(45));

    // 无显式 TTL 写入 → 继承默认 TTL
    CacheWriter::set(&backend, Arc::from("d"), Arc::new(b"1".to_vec()), None)
        .await
        .unwrap();
    let ttl = CacheReader::ttl(&backend, "d").await.unwrap().expect("ttl");
    assert!(
        ttl <= Duration::from_secs(45) && ttl > Duration::from_secs(40),
        "ttl={ttl:?}"
    );
    CacheConnector::shutdown(&backend).await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn lazy_expiry_physically_removes_on_read() {
    let path = temp_path("lazy");
    let backend = RedbDiskBackend::create(&path).unwrap();

    CacheWriter::set(
        &backend,
        Arc::from("gone"),
        Arc::new(b"v".to_vec()),
        Some(Duration::from_millis(30)),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(80)).await;

    // 懒过期：读取返回 None 并触发物理删除
    assert!(CacheReader::get(&backend, "gone").await.unwrap().is_none());

    let stats = CacheReader::stats(&backend).await.unwrap();
    assert_eq!(
        stats.get("path").map(String::as_str),
        Some(path.to_str().unwrap())
    );
    assert_eq!(stats.get("len").map(String::as_str), Some("0"));
    CacheConnector::shutdown(&backend).await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn delete_expire_exists_len_surface() {
    let path = temp_path("surface");
    let backend = RedbDiskBackend::create(&path).unwrap();

    CacheWriter::set(&backend, Arc::from("a"), Arc::new(b"1".to_vec()), None)
        .await
        .unwrap();
    CacheWriter::set(&backend, Arc::from("b"), Arc::new(b"2".to_vec()), None)
        .await
        .unwrap();
    assert_eq!(CacheReader::len(&backend).await.unwrap(), 2);
    assert!(CacheReader::exists(&backend, "a").await.unwrap());

    // expire 命中与未命中
    assert!(
        CacheWriter::expire(&backend, "a", Duration::from_secs(60))
            .await
            .unwrap()
    );
    assert!(
        !CacheWriter::expire(&backend, "ghost", Duration::from_secs(60))
            .await
            .unwrap()
    );

    CacheWriter::delete(&backend, "a").await.unwrap();
    assert!(CacheReader::get(&backend, "a").await.unwrap().is_none());

    CacheWriter::clear(&backend).await.unwrap();
    assert_eq!(CacheReader::len(&backend).await.unwrap(), 0);
    CacheConnector::shutdown(&backend).await;
    let _ = std::fs::remove_file(&path);
}
