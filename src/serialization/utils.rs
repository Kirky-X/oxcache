//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 序列化工具模块
//!
//! 提供序列化相关的共享工具函数。

use crate::error::{CacheError, Result};

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
/// * `Err(CacheError)` - 数据大小超过限制
pub fn check_data_size(data: &[u8], max_size: usize, data_type: &str) -> Result<()> {
    if data.len() > max_size {
        return Err(CacheError::Serialization(format!(
            "{} data too large: {} bytes (max: {} bytes)",
            data_type,
            data.len(),
            max_size
        )));
    }
    Ok(())
}

/// 最小压缩阈值 - 小于此大小的数据不压缩
const MIN_COMPRESS_SIZE: usize = 100;

/// 使用flate2压缩数据
///
/// 根据数据大小智能选择压缩策略：
/// - 小于100字节：直接返回原数据（压缩开销不划算）
/// - 100-1KB：使用快速压缩（Compression::fast）
/// - 1KB-100KB：使用中等压缩（Compression::new(6)）
/// - 大于100KB：使用高压缩率（Compression::best）
#[cfg(feature = "flate2")]
pub fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
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
        .map_err(|e| CacheError::Serialization(e.to_string()))?;
    encoder
        .finish()
        .map_err(|e| CacheError::Serialization(e.to_string()))
}

/// 使用flate2解压缩数据
#[cfg(feature = "flate2")]
pub fn decompress_data(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data);
    let mut decoded = Vec::new();
    decoder
        .read_to_end(&mut decoded)
        .map_err(|e| CacheError::Serialization(e.to_string()))?;
    Ok(decoded)
}

/// 当flate2特性未启用时的压缩函数（直接返回原数据）
#[cfg(not(feature = "flate2"))]
pub fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}

/// 当flate2特性未启用时的解压缩函数（直接返回原数据）
#[cfg(not(feature = "flate2"))]
pub fn decompress_data(data: &[u8]) -> Result<Vec<u8>> {
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
}
