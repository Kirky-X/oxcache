// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Redis 命令枚举

/// Redis 命令枚举
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RedisCommand {
    Ping,
    Get,
    Set,
    SetEx,
    Del,
    Exists,
    Expire,
    Ttl,
    Scan,
    Keys,
    Dbsize,
    Info,
    FlushAll,
    FlushDb,
    Shutdown,
    Debug,
    Config,
    Save,
    BgSave,
    BgRewriteAof,
    SlaveOf,
    ReplicaOf,
    Cluster,
    Admin,
    Monitor,
    Eval,
    EvalSha,
    Script,
}

impl RedisCommand {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ping => "PING",
            Self::Get => "GET",
            Self::Set => "SET",
            Self::SetEx => "SETEX",
            Self::Del => "DEL",
            Self::Exists => "EXISTS",
            Self::Expire => "EXPIRE",
            Self::Ttl => "TTL",
            Self::Scan => "SCAN",
            Self::Keys => "KEYS",
            Self::Dbsize => "DBSIZE",
            Self::Info => "INFO",
            Self::FlushAll => "FLUSHALL",
            Self::FlushDb => "FLUSHDB",
            Self::Shutdown => "SHUTDOWN",
            Self::Debug => "DEBUG",
            Self::Config => "CONFIG",
            Self::Save => "SAVE",
            Self::BgSave => "BGSAVE",
            Self::BgRewriteAof => "BGREWRITEAOF",
            Self::SlaveOf => "SLAVEOF",
            Self::ReplicaOf => "REPLICAOF",
            Self::Cluster => "CLUSTER",
            Self::Admin => "ADMIN",
            Self::Monitor => "MONITOR",
            Self::Eval => "EVAL",
            Self::EvalSha => "EVALSHA",
            Self::Script => "SCRIPT",
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
            RedisCommand::SetEx,
            RedisCommand::Del,
            RedisCommand::Exists,
            RedisCommand::Expire,
            RedisCommand::Ttl,
            RedisCommand::Scan,
            RedisCommand::Keys,
            RedisCommand::Dbsize,
            RedisCommand::Info,
            RedisCommand::FlushAll,
            RedisCommand::FlushDb,
            RedisCommand::Shutdown,
            RedisCommand::Debug,
            RedisCommand::Config,
            RedisCommand::Save,
            RedisCommand::BgSave,
            RedisCommand::BgRewriteAof,
            RedisCommand::SlaveOf,
            RedisCommand::ReplicaOf,
            RedisCommand::Cluster,
            RedisCommand::Admin,
            RedisCommand::Monitor,
            RedisCommand::Eval,
            RedisCommand::EvalSha,
            RedisCommand::Script,
        ];
        for cmd in &variants {
            assert!(!cmd.as_str().is_empty(), "Command {:?} has empty as_str()", cmd);
        }
    }

    #[test]
    fn test_as_str_returns_uppercase() {
        assert_eq!(RedisCommand::Ping.as_str(), "PING");
        assert_eq!(RedisCommand::Get.as_str(), "GET");
        assert_eq!(RedisCommand::Set.as_str(), "SET");
        assert_eq!(RedisCommand::SetEx.as_str(), "SETEX");
        assert_eq!(RedisCommand::Del.as_str(), "DEL");
        assert_eq!(RedisCommand::Exists.as_str(), "EXISTS");
        assert_eq!(RedisCommand::Expire.as_str(), "EXPIRE");
        assert_eq!(RedisCommand::Ttl.as_str(), "TTL");
        assert_eq!(RedisCommand::Scan.as_str(), "SCAN");
        assert_eq!(RedisCommand::Keys.as_str(), "KEYS");
        assert_eq!(RedisCommand::Dbsize.as_str(), "DBSIZE");
        assert_eq!(RedisCommand::Info.as_str(), "INFO");
        assert_eq!(RedisCommand::FlushAll.as_str(), "FLUSHALL");
        assert_eq!(RedisCommand::FlushDb.as_str(), "FLUSHDB");
    }
}
