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

/// 使用flate2压缩数据
#[cfg(feature = "flate2")]
pub fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
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
    fn test_compress_data_no_feature() {
        let data = b"hello world";
        let compressed = compress_data(data).unwrap();
        assert_eq!(compressed, data);
    }

    #[test]
    fn test_decompress_data_no_feature() {
        let data = b"hello world";
        let decompressed = decompress_data(data).unwrap();
        assert_eq!(decompressed, data);
    }
}
