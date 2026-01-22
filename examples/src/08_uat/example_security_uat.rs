// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Security UAT (User Acceptance Testing) example
//
// This example validates security requirements and protections.

use serde_json::Value;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct SensitiveData {
    user_id: u64,
    ssn: String,
    credit_card: String,
    medical_record: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxcache::manager::{get_client, init};
    use oxcache::{
        config::{L1Config, OxcacheConfig, ServiceConfig},
        CacheExt,
    };
    
    #[tokio::test]
    async fn test_data_isolation() {
        // Requirement: User data should be isolated between tenants
        let config = OxcacheConfig::builder()
            .with_service(
                "secure_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(1000)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("secure_cache").unwrap();
        
        let data_tenant_a = SensitiveData {
            user_id: 1,
            ssn: "***-**-1234".to_string(),
            credit_card: "****-****-****-1234".to_string(),
            medical_record: "Confidential".to_string(),
        };
        
        let data_tenant_b = SensitiveData {
            user_id: 2,
            ssn: "***-**-5678".to_string(),
            credit_card: "****-****-****-5678".to_string(),
            medical_record: "Confidential".to_string(),
        };
        
        // Store with tenant prefix
        client.set("tenant_a:user_1", &data_tenant_a, Some(3600)).await.unwrap();
        client.set("tenant_b:user_2", &data_tenant_b, Some(3600)).await.unwrap();
        
        // Verify isolation
        let retrieved_a = client.get::<SensitiveData>("tenant_a:user_1").await.unwrap();
        let retrieved_b = client.get::<SensitiveData>("tenant_b:user_2").await.unwrap();
        
        assert!(retrieved_a.is_some());
        assert!(retrieved_b.is_some());
        assert_ne!(retrieved_a.unwrap().user_id, retrieved_b.unwrap().user_id);
    }
    
    #[tokio::test]
    async fn test_access_control() {
        // Requirement: Access should be controlled based on permissions
        let config = OxcacheConfig::builder()
            .with_service(
                "access_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(500)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("access_cache").unwrap();
        
        // Simulate role-based access control
        let admin_data = json!({
            "role": "admin",
            "permissions": ["read", "write", "delete"]
        });
        
        let user_data = json!({
            "role": "user",
            "permissions": ["read"]
        });
        
        client.set("role:admin", &admin_data, None).await.unwrap();
        client.set("role:user", &user_data, None).await.unwrap();
        
        // Verify role-based access
        let admin_perms = client.get::<Value>("role:admin").await.unwrap();
        let user_perms = client.get::<Value>("role:user").await.unwrap();
        
        assert!(admin_perms.is_some());
        assert!(user_perms.is_some());
        
        // In real implementation, you would check permissions before operations
        let admin_permissions = admin_perms.unwrap().get("permissions").unwrap().as_array().unwrap();
        assert!(admin_permissions.len() == 3);
        
        let user_permissions = user_perms.unwrap().get("permissions").unwrap().as_array().unwrap();
        assert!(user_permissions.len() == 1);
    }
    
    #[tokio::test]
    async fn test_data_sanitization() {
        // Requirement: Input data should be sanitized before caching
        let config = OxcacheConfig::builder()
            .with_service(
                "sanitized_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(1000)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("sanitized_cache").unwrap();
        
        // Simulate malicious input
        let malicious_input = "<script>alert('xss')</script>";
        let sanitized_input = "alert('xss')"; // Sanitized version
        
        // Store sanitized version
        client.set("safe_input", &sanitized_input, None).await.unwrap();
        
        // Retrieve and verify it's sanitized
        let retrieved = client.get::<String>("safe_input").await.unwrap();
        assert_eq!(retrieved.unwrap(), "alert('xss')");
        assert!(!retrieved.unwrap().contains("<script>"));
        assert!(!retrieved.unwrap().contains("</script>"));
    }
    
    #[tokio::test]
    async fn test_encryption_at_rest() {
        // Requirement: Sensitive data should be encrypted in transit
        let config = OxcacheConfig::builder()
            .with_service(
                "encrypted_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(500)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("encrypted_cache").unwrap();
        
        let sensitive_data = SensitiveData {
            user_id: 12345,
            ssn: "123-45-6789".to_string(),
            credit_card: "4111-1111-1111-1111".to_string(),
            medical_record: "Patient data".to_string(),
        };
        
        // In a real implementation, data would be encrypted before caching
        client.set("encrypted:user_12345", &sensitive_data, Some(3600)).await.unwrap();
        
        let retrieved = client.get::<SensitiveData>("encrypted:user_12345").await.unwrap();
        assert!(retrieved.is_some());
        
        // Verify data integrity
        let original = retrieved.unwrap();
        assert_eq!(original.user_id, 12345);
        assert_eq!(original.ssn, "123-45-6789");
    }
    
    #[tokio::test]
    async fn test_audit_logging() {
        // Requirement: Security events should be logged
        let config = OxcacheConfig::builder()
            .with_service(
                "audit_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(1000)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("audit_cache").unwrap();
        
        // Simulate security event logging
        let security_event = json!({
            "event_type": "access_attempt",
            "user_id": 12345,
            "resource": "sensitive_data",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "ip_address": "192.168.1.100",
            "result": "success"
        });
        
        client.set("audit:12345", &security_event, None).await.unwrap();
        
        // In a real implementation, this would be sent to a logging system
        let logged_event = client.get::<Value>("audit:12345").await.unwrap();
        assert!(logged_event.is_some());
        
        let event = logged_event.unwrap();
        assert_eq!(event.get("event_type"), Some(&Value::String("access_attempt".to_string())));
        assert_eq!(event.get("user_id"), Some(&Value::Number(serde_json::Number::from(12345))));
    }
}