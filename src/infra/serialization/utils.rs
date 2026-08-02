// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 序列化工具模块
//!
//! 提供序列化相关的共享工具函数。

use crate::error::{OxCacheError, OxCacheResult};

/// 检查数据大小是否超过限制
///
/// # 参数
///
/// * `data` - 要检查的数据
/// * `max_size` - 最大允许大小
/// * `data_type` - 数据类型描述（用于错误消息）
///
/// # 返回值
///
/// * `Ok(())` - 数据大小在限制内
/// * `Err(OxCacheError)` - 数据大小超过限制
pub fn check_data_size(data: &[u8], max_size: usize, data_type: &str) -> OxCacheResult<()> {
    if data.len() > max_size {
        return Err(OxCacheError::Serialization(format!(
            "{} data too large: {} bytes (max: {} bytes)",
            data_type,
            data.len(),
            max_size
        )));
    }
    Ok(())
}

/// 最小压缩阈值 - 小于此时长的数据不压缩
#[cfg(feature = "flate2")]
const MIN_COMPRESS_SIZE: usize = 100;

/// 解压最大输出大小限制（防止恶意 gzip 数据解压炸弹导致 OOM）
pub const MAX_DECOMPRESS_SIZE: usize = 64 * 1024 * 1024;

/// gzip 魔数（0x1f 0x8b），用于区分已压缩与未压缩数据
#[cfg(feature = "flate2")]
fn is_gzip(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b
}

/// 使用flate2压缩数据
///
/// 根据数据大小智能选择压缩策略：
/// - 小于100字节：直接返回原数据（压缩开销不划算）
/// - 100-1KB：使用快速压缩（Compression::fast）
/// - 1KB-100KB：使用中等压缩（Compression::new(6)）
/// - 大于100KB：使用高压缩率（Compression::best）
///
/// 压缩后会检查压缩比：若压缩结果不小于原始数据（如已压缩/随机数据），
/// 则返回原始数据（未压缩），避免存储膨胀。
#[cfg(feature = "flate2")]
pub fn compress_data(data: &[u8]) -> OxCacheResult<Vec<u8>> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    // 小数据不压缩，避免压缩开销
    if data.len() < MIN_COMPRESS_SIZE {
        return Ok(data.to_vec());
    }

    // 根据数据大小选择压缩级别
    let compression = if data.len() < 1024 {
        // 100B - 1KB: 快速压缩
        Compression::fast()
    } else if data.len() < 100 * 1024 {
        // 1KB - 100KB: 中等压缩 (级别6)
        Compression::new(6)
    } else {
        // >100KB: 高压缩率
        Compression::best()
    };

    let mut encoder = GzEncoder::new(Vec::new(), compression);
    encoder
        .write_all(data)
        .map_err(|e| OxCacheError::Serialization(e.to_string()))?;
    let compressed = encoder.finish().map_err(|e| OxCacheError::Serialization(e.to_string()))?;

    // 压缩比检查：压缩后不小于原数据则返回原数据（未压缩）
    if compressed.len() >= data.len() {
        return Ok(data.to_vec());
    }
    Ok(compressed)
}

/// 使用flate2解压缩数据（限制最大输出大小，防止 OOM）
///
/// 非 gzip 数据（未压缩）直接返回原数据；gzip 数据解压时通过
/// `Take` 限制读取字节数，超过 `max_size` 返回错误。
#[cfg(feature = "flate2")]
pub fn decompress_data_with_limit(data: &[u8], max_size: usize) -> OxCacheResult<Vec<u8>> {
    if !is_gzip(data) {
        return Ok(data.to_vec());
    }
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data).take(max_size as u64 + 1);
    let mut decoded = Vec::new();
    decoder
        .read_to_end(&mut decoded)
        .map_err(|e| OxCacheError::Serialization(e.to_string()))?;
    if decoded.len() > max_size {
        return Err(OxCacheError::Serialization(format!(
            "decompressed data too large: {} bytes (max: {} bytes)",
            decoded.len(),
            max_size
        )));
    }
    Ok(decoded)
}

/// 使用flate2解压缩数据（默认限制输出大小）
#[cfg(feature = "flate2")]
pub fn decompress_data(data: &[u8]) -> OxCacheResult<Vec<u8>> {
    decompress_data_with_limit(data, MAX_DECOMPRESS_SIZE)
}

/// 当flate2特性未启用时的压缩函数（直接返回原数据）
#[cfg(not(feature = "flate2"))]
pub fn compress_data(data: &[u8]) -> OxCacheResult<Vec<u8>> {
    Ok(data.to_vec())
}

/// 当flate2特性未启用时的解压缩函数（直接返回原数据）
#[cfg(not(feature = "flate2"))]
pub fn decompress_data(data: &[u8]) -> OxCacheResult<Vec<u8>> {
    Ok(data.to_vec())
}

/// 当flate2特性未启用时的受限解压缩函数（直接返回原数据）
#[cfg(not(feature = "flate2"))]
pub fn decompress_data_with_limit(data: &[u8], _max_size: usize) -> OxCacheResult<Vec<u8>> {
    Ok(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_data_size_within_limit() {
        let data = vec![0u8; 1000];
        assert!(check_data_size(&data, 2000, "test").is_ok());
    }

    #[test]
    fn test_check_data_size_exceeds_limit() {
        let data = vec![0u8; 3000];
        assert!(check_data_size(&data, 2000, "test").is_err());
    }

    #[test]
    #[cfg(not(feature = "flate2"))]
    fn test_compress_data_no_feature() {
        let data = b"hello world";
        let compressed = compress_data(data).unwrap();
        assert_eq!(compressed, data);
    }

    #[test]
    #[cfg(not(feature = "flate2"))]
    fn test_decompress_data_no_feature() {
        let data = b"hello world";
        let decompressed = decompress_data(data).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    #[cfg(feature = "flate2")]
    fn test_compress_data_with_feature() {
        // 使用大于100字节的数据测试压缩
        let data = vec![0u8; 200]; // 200字节
        let compressed = compress_data(&data).unwrap();
        // 压缩后的数据应该与原数据不同
        assert_ne!(compressed, data);
        // 解压后应该得到原数据
        let decompressed = decompress_data(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    #[cfg(feature = "flate2")]
    fn test_decompress_data_with_feature() {
        let data = vec![0u8; 200]; // 200字节
        let compressed = compress_data(&data).unwrap();
        let decompressed = decompress_data(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    #[cfg(feature = "flate2")]
    fn test_small_data_not_compressed() {
        // 小于100字节的数据不应该被压缩
        let data = b"small data";
        let compressed = compress_data(data).unwrap();
        assert_eq!(compressed, data);
    }

    #[test]
    #[cfg(feature = "flate2")]
    fn test_compress_ratio_check_incompressible_data() {
        // 随机/已压缩数据压缩后不应大于原数据
        let data: Vec<u8> = (0..200u32).map(|i| (i * 31 % 256) as u8).collect();
        let compressed = compress_data(&data).unwrap();
        // 不可压缩数据应返回原数据（或至少不超过原大小）
        assert!(compressed.len() <= data.len());
        // 往返一致
        let decompressed = decompress_data(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    #[cfg(feature = "flate2")]
    fn test_compress_ratio_check_compressed_input() {
        // 对已压缩的 gzip 数据再次压缩不应膨胀
        let data = vec![0u8; 500];
        let first = compress_data(&data).unwrap();
        let second = compress_data(&first).unwrap();
        assert!(second.len() <= first.len());
        let decompressed = decompress_data(&second).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    #[cfg(feature = "flate2")]
    fn test_decompress_data_with_limit_exceeds() {
        // 构造高压缩比数据，验证解压大小限制生效
        let data = vec![0u8; 1024 * 1024]; // 1MB 零字节
        let compressed = compress_data(&data).unwrap();
        assert!(compressed.len() < data.len());
        // 限制远小于实际解压大小，应报错
        let result = decompress_data_with_limit(&compressed, 1024);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "flate2")]
    fn test_decompress_data_with_limit_within() {
        let data = vec![0u8; 5000];
        let compressed = compress_data(&data).unwrap();
        let decompressed = decompress_data_with_limit(&compressed, 64 * 1024).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    #[cfg(feature = "flate2")]
    fn test_decompress_data_with_limit_uncompressed_passthrough() {
        // 非 gzip 数据直接透传
        let data = b"not compressed data at all";
        let result = decompress_data_with_limit(data, 1024).unwrap();
        assert_eq!(result, data);
    }
}
