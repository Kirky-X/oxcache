// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis 命令枚举

/// Redis 命令枚举
///
/// 定义 Redis 数据路径使用的命令变体，经 `as_str` 映射为协议命令名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RedisCommand {
    Ping,
    Get,
    Set,
    Del,
    Exists,
    PExpire,
    Ttl,
    Scan,
    Dbsize,
    Info,
    Eval,
    EvalSha,
    Script,
    Incr,
    IncrBy,
}

impl RedisCommand {
    /// 返回命令的 Redis 协议字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ping => "PING",
            Self::Get => "GET",
            Self::Set => "SET",
            Self::Del => "DEL",
            Self::Exists => "EXISTS",
            Self::PExpire => "PEXPIRE",
            Self::Ttl => "TTL",
            Self::Scan => "SCAN",
            Self::Dbsize => "DBSIZE",
            Self::Info => "INFO",
            Self::Eval => "EVAL",
            Self::EvalSha => "EVALSHA",
            Self::Script => "SCRIPT",
            Self::Incr => "INCR",
            Self::IncrBy => "INCRBY",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_commands_have_non_empty_str() {
        // Test all variants return a non-empty string
        let variants = [
            RedisCommand::Ping,
            RedisCommand::Get,
            RedisCommand::Set,
            RedisCommand::Del,
            RedisCommand::Exists,
            RedisCommand::PExpire,
            RedisCommand::Ttl,
            RedisCommand::Scan,
            RedisCommand::Dbsize,
            RedisCommand::Info,
            RedisCommand::Eval,
            RedisCommand::EvalSha,
            RedisCommand::Script,
            RedisCommand::Incr,
            RedisCommand::IncrBy,
        ];
        for cmd in &variants {
            assert!(
                !cmd.as_str().is_empty(),
                "Command {:?} has empty as_str()",
                cmd
            );
        }
    }

    #[test]
    fn test_as_str_returns_uppercase() {
        assert_eq!(RedisCommand::Ping.as_str(), "PING");
        assert_eq!(RedisCommand::Get.as_str(), "GET");
        assert_eq!(RedisCommand::Set.as_str(), "SET");
        assert_eq!(RedisCommand::Del.as_str(), "DEL");
        assert_eq!(RedisCommand::Exists.as_str(), "EXISTS");
        assert_eq!(RedisCommand::PExpire.as_str(), "PEXPIRE");
        assert_eq!(RedisCommand::Ttl.as_str(), "TTL");
        assert_eq!(RedisCommand::Scan.as_str(), "SCAN");
        assert_eq!(RedisCommand::Dbsize.as_str(), "DBSIZE");
        assert_eq!(RedisCommand::Info.as_str(), "INFO");
    }
}
