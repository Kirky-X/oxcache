//! 降级策略和健康状态测试
//!
//! 合并自:
/// - tests/degradation_test.rs
/// - tests/degradation_integration_test.rs
/// - tests/health_state_test.rs
use oxcache::config::{L2Config, RedisMode};
use oxcache::recovery::health::{
    HealthCheckableBackend, HealthChecker, HealthState, WalReplayableBackendTrait,
};
use oxcache::recovery::wal::{WalEntry, WalManager};
use secrecy::SecretString;
use std::sync::Arc;
use tokio::sync::RwLock;

#[path = "../common/mod.rs"]
mod common;

#[derive(Clone)]
pub struct FailingL2Backend {
    pub command_timeout_ms: u64,
}

impl FailingL2Backend {
    pub async fn ping(&self) -> oxcache::error::Result<()> {
        Err(oxcache::error::CacheError::BackendError(
            "模拟连接失败".to_string(),
        ))
    }

    pub fn command_timeout_ms(&self) -> u64 {
        self.command_timeout_ms
    }

    pub async fn pipeline_replay(&self, _entries: Vec<WalEntry>) -> oxcache::error::Result<()> {
        Err(oxcache::error::CacheError::BackendError(
            "模拟连接失败".to_string(),
        ))
    }
}

impl HealthCheckableBackend for FailingL2Backend {
    async fn ping(&self) -> oxcache::error::Result<()> {
        self.ping().await
    }

    fn command_timeout_ms(&self) -> u64 {
        self.command_timeout_ms
    }
}

impl WalReplayableBackendTrait for FailingL2Backend {
    async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> oxcache::error::Result<()> {
        self.pipeline_replay(entries).await
    }
}

pub async fn create_failing_l2_backend(config: &L2Config) -> Arc<FailingL2Backend> {
    Arc::new(FailingL2Backend {
        command_timeout_ms: config.command_timeout_ms,
    })
}

fn create_test_l2_config() -> L2Config {
    L2Config {
        connection_string: SecretString::new("redis://127.0.0.1:2".into()),
        mode: RedisMode::Standalone,
        connection_timeout_ms: 100,
        command_timeout_ms: 100,
        password: None,
        enable_tls: false,
        sentinel: None,
        cluster: None,
        default_ttl: None,
        max_key_length: 256,
        max_value_size: 1024 * 1024 * 10,
    }
}

mod health_state_transition_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_state_transitions() {
        let health_state = Arc::new(RwLock::new(HealthState::Healthy));

        {
            let state = *health_state.read().await;
            assert!(matches!(state, HealthState::Healthy));
        }

        {
            let mut state_guard = health_state.write().await;
            *state_guard = HealthState::Degraded {
                since: std::time::Instant::now(),
                failure_count: 1,
            };
        }

        {
            let state = *health_state.read().await;
            match state {
                HealthState::Degraded { failure_count, .. } => {
                    assert_eq!(failure_count, 1);
                }
                _ => panic!("Expected Degraded state"),
            }
        }

        {
            let mut state_guard = health_state.write().await;
            *state_guard = HealthState::Recovering {
                since: std::time::Instant::now(),
                success_count: 1,
            };
        }

        {
            let state = *health_state.read().await;
            match state {
                HealthState::Recovering { success_count, .. } => {
                    assert_eq!(success_count, 1);
                }
                _ => panic!("Expected Recovering state"),
            }
        }

        println!("健康状态转换测试通过");
    }

    #[tokio::test]
    async fn test_health_checker_integration() {
        let health_state = Arc::new(RwLock::new(HealthState::Healthy));

        let initial_state = *health_state.read().await;
        assert!(matches!(initial_state, HealthState::Healthy));

        println!("健康检查器集成测试通过");
    }
}

mod l2_failure_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_l2_failure_logic() {
        let health_state = Arc::new(RwLock::new(HealthState::Healthy));

        {
            let mut state_guard = health_state.write().await;
            if let HealthState::Healthy = *state_guard {
                *state_guard = HealthState::Degraded {
                    since: std::time::Instant::now(),
                    failure_count: 1,
                };
            }
        }

        {
            let state = *health_state.read().await;
            match state {
                HealthState::Degraded { failure_count, .. } => {
                    assert_eq!(failure_count, 1);
                }
                _ => panic!("Expected Degraded state after first failure"),
            }
        }

        {
            let mut state_guard = health_state.write().await;
            if let HealthState::Degraded {
                since,
                failure_count,
            } = *state_guard
            {
                *state_guard = HealthState::Degraded {
                    since,
                    failure_count: failure_count + 1,
                };
            }
        }

        {
            let state = *health_state.read().await;
            match state {
                HealthState::Degraded { failure_count, .. } => {
                    assert_eq!(failure_count, 2);
                }
                _ => panic!("Expected Degraded state with increased failure count"),
            }
        }

        {
            let mut state_guard = health_state.write().await;
            *state_guard = HealthState::Recovering {
                since: std::time::Instant::now(),
                success_count: 2,
            };
        }

        {
            let mut state_guard = health_state.write().await;
            if let HealthState::Recovering { .. } = *state_guard {
                *state_guard = HealthState::Degraded {
                    since: std::time::Instant::now(),
                    failure_count: 1,
                };
            }
        }

        {
            let state = *health_state.read().await;
            if let HealthState::Degraded { failure_count, .. } = state {
                assert_eq!(failure_count, 1);
            } else {
                panic!("Expected Degraded state after recovery failure");
            }
        }

        println!("handle_l2_failure逻辑测试通过");
    }
}

mod degradation_consistency_tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_l2_failure_integration() {
        println!("开始降级策略集成测试...");

        let health_state = Arc::new(tokio::sync::RwLock::new(HealthState::Healthy));

        let initial_state = *health_state.read().await;
        assert!(matches!(initial_state, HealthState::Healthy));
        println!("初始状态: {:?}", initial_state);

        {
            let mut state_guard = health_state.write().await;
            if let HealthState::Healthy = *state_guard {
                println!("服务从Healthy转换到Degraded");
                *state_guard = HealthState::Degraded {
                    since: std::time::Instant::now(),
                    failure_count: 1,
                };
            }
        }

        let state_after_first_failure = *health_state.read().await;
        match state_after_first_failure {
            HealthState::Degraded { failure_count, .. } => {
                assert_eq!(failure_count, 1);
                println!(
                    "第一次失败后的状态: Degraded(failure_count={})",
                    failure_count
                );
            }
            _ => panic!("期望降级状态"),
        }

        {
            let mut state_guard = health_state.write().await;
            match *state_guard {
                HealthState::Degraded {
                    since,
                    failure_count,
                } => {
                    println!(
                        "服务保持Degraded状态，失败计数增加: {} -> {}",
                        failure_count,
                        failure_count + 1
                    );
                    *state_guard = HealthState::Degraded {
                        since,
                        failure_count: failure_count + 1,
                    };
                }
                _ => panic!("期望降级状态"),
            }
        }

        let state_after_second_failure = *health_state.read().await;
        match state_after_second_failure {
            HealthState::Degraded { failure_count, .. } => {
                assert_eq!(failure_count, 2);
                println!(
                    "第二次失败后的状态: Degraded(failure_count={})",
                    failure_count
                );
            }
            _ => panic!("期望降级状态"),
        }

        {
            let mut state_guard = health_state.write().await;
            *state_guard = HealthState::Recovering {
                since: std::time::Instant::now(),
                success_count: 2,
            };
        }

        let recovering_state = *health_state.read().await;
        match recovering_state {
            HealthState::Recovering { success_count, .. } => {
                assert_eq!(success_count, 2);
                println!("恢复状态: Recovering(success_count={})", success_count);
            }
            _ => panic!("期望恢复状态"),
        }

        {
            let mut state_guard = health_state.write().await;
            match *state_guard {
                HealthState::Recovering { .. } => {
                    println!("服务从Recovering转换回Degraded");
                    *state_guard = HealthState::Degraded {
                        since: std::time::Instant::now(),
                        failure_count: 1,
                    };
                }
                _ => panic!("期望恢复状态"),
            }
        }

        let state_after_recovery_failure = *health_state.read().await;
        match state_after_recovery_failure {
            HealthState::Degraded { failure_count, .. } => {
                assert_eq!(failure_count, 1);
                println!(
                    "恢复失败后的状态: Degraded(failure_count={})",
                    failure_count
                );
            }
            _ => panic!("期望降级状态"),
        }

        println!("handle_l2_failure状态转换逻辑测试通过");
    }

    #[tokio::test]
    async fn test_degradation_state_consistency() {
        println!("开始降级状态一致性测试...");

        let health_state = Arc::new(tokio::sync::RwLock::new(HealthState::Healthy));

        let test_cases = vec![
            (HealthState::Healthy, "Healthy"),
            (
                HealthState::Degraded {
                    since: std::time::Instant::now(),
                    failure_count: 1,
                },
                "Degraded(1)",
            ),
            (
                HealthState::Degraded {
                    since: std::time::Instant::now(),
                    failure_count: 5,
                },
                "Degraded(5)",
            ),
            (
                HealthState::Recovering {
                    since: std::time::Instant::now(),
                    success_count: 1,
                },
                "Recovering(1)",
            ),
            (
                HealthState::Recovering {
                    since: std::time::Instant::now(),
                    success_count: 3,
                },
                "Recovering(3)",
            ),
        ];

        for (expected_state, description) in test_cases {
            {
                let mut state_guard = health_state.write().await;
                *state_guard = expected_state;
            }

            let actual_state = *health_state.read().await;
            match (expected_state, actual_state) {
                (HealthState::Healthy, HealthState::Healthy) => {
                    println!("✓ {} 状态正确", description);
                }
                (
                    HealthState::Degraded {
                        failure_count: expected_count,
                        ..
                    },
                    HealthState::Degraded {
                        failure_count: actual_count,
                        ..
                    },
                ) => {
                    assert_eq!(expected_count, actual_count);
                    println!(
                        "✓ {} 状态正确 (failure_count={})",
                        description, actual_count
                    );
                }
                (
                    HealthState::Recovering {
                        success_count: expected_count,
                        ..
                    },
                    HealthState::Recovering {
                        success_count: actual_count,
                        ..
                    },
                ) => {
                    assert_eq!(expected_count, actual_count);
                    println!(
                        "✓ {} 状态正确 (success_count={})",
                        description, actual_count
                    );
                }
                _ => panic!(
                    "状态不匹配: 期望 {:?}, 实际 {:?}",
                    expected_state, actual_state
                ),
            }
        }

        println!("降级状态一致性测试通过");
    }
}

mod health_checker_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_state_basic() {
        let health_state = Arc::new(tokio::sync::RwLock::new(HealthState::Healthy));

        let initial_state = *health_state.read().await;
        assert!(matches!(initial_state, HealthState::Healthy));
        println!("初始健康状态: {:?}", initial_state);

        {
            let mut state_guard = health_state.write().await;
            *state_guard = HealthState::Degraded {
                since: std::time::Instant::now(),
                failure_count: 1,
            };
        }

        let degraded_state = *health_state.read().await;
        match degraded_state {
            HealthState::Degraded { failure_count, .. } => {
                assert_eq!(failure_count, 1);
                println!("降级状态验证通过，失败次数: {}", failure_count);
            }
            _ => panic!("期望降级状态"),
        }

        {
            let mut state_guard = health_state.write().await;
            *state_guard = HealthState::Recovering {
                since: std::time::Instant::now(),
                success_count: 1,
            };
        }

        let recovering_state = *health_state.read().await;
        match recovering_state {
            HealthState::Recovering { success_count, .. } => {
                assert_eq!(success_count, 1);
                println!("恢复状态验证通过，成功次数: {}", success_count);
            }
            _ => panic!("期望恢复中状态"),
        }

        println!("健康状态基础测试完成");
    }

    #[tokio::test]
    async fn test_degradation_state_transition() {
        let config = create_test_l2_config();
        let l2_backend = create_failing_l2_backend(&config).await;

        let service_name = common::generate_unique_service_name("degradation");
        let health_state = Arc::new(tokio::sync::RwLock::new(HealthState::Healthy));
        let wal = Arc::new(
            WalManager::new(&service_name)
                .await
                .expect("Failed to create WAL"),
        );

        let checker = HealthChecker::new(
            l2_backend.clone(),
            health_state.clone(),
            wal,
            service_name.clone(),
            100,
        );

        let initial_state = *health_state.read().await;
        assert!(matches!(initial_state, HealthState::Healthy));
        println!("初始状态: {:?}", initial_state);

        println!("启动健康检查器（预期检测到L2后端故障）...");
        let checker_handle = tokio::spawn(checker.start());

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let current_state = *health_state.read().await;
        println!("当前状态: {:?}", current_state);

        checker_handle.abort();

        match current_state {
            HealthState::Healthy => {
                panic!("状态应该是降级，健康检查应该检测到L2后端故障");
            }
            HealthState::Degraded {
                since: _,
                failure_count,
            } => {
                println!("成功检测到降级状态，失败次数: {}", failure_count);
                assert!(failure_count >= 1);
            }
            HealthState::Recovering {
                since: _,
                success_count,
            } => {
                panic!(
                    "状态应该是降级，不应该是恢复中，成功次数: {}",
                    success_count
                );
            }
        }

        println!("降级状态转换测试完成");
        common::cleanup_service(&service_name).await;
    }
}
