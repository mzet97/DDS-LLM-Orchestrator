//! Testes do cache local de políticas (dashmap + TTL — análogo `_NullRedis`).

use std::time::Duration;

use policy_engine::cache::PolicyCache;
use serde_json::json;

#[test]
fn set_get_delete() {
    let cache = PolicyCache::new();
    assert!(cache.get("default").is_none());
    assert!(cache.is_empty());

    cache.set("default", json!({"version": 2}), Duration::from_secs(300));
    assert_eq!(cache.get("default"), Some(json!({"version": 2})));
    assert_eq!(cache.len(), 1);

    assert!(cache.delete("default"));
    assert!(cache.get("default").is_none());
    assert!(!cache.delete("default"), "delete de ausente retorna false");
}

#[test]
fn ttl_expira_e_remove_entrada() {
    let cache = PolicyCache::new();
    let now = 1_000_000_u64;
    cache.set_at("p", json!({"version": 1}), Duration::from_millis(100), now);

    assert!(cache.get_at("p", now + 99).is_some(), "dentro do TTL");
    assert!(
        cache.get_at("p", now + 100).is_none(),
        "no limite do TTL já expirou"
    );
    assert_eq!(cache.len(), 0, "entrada expirada é removida no acesso");
}

#[test]
fn sobrescrever_renova_ttl() {
    let cache = PolicyCache::new();
    let now = 1_000_000_u64;
    cache.set_at("p", json!(1), Duration::from_millis(100), now);
    cache.set_at("p", json!(2), Duration::from_millis(100), now + 50);
    assert_eq!(cache.get_at("p", now + 120), Some(json!(2)));
}

#[test]
fn cache_real_com_clock_do_sistema() {
    // Smoke com o clock real (o caminho usado pelo serviço).
    let cache = PolicyCache::new();
    cache.set("p", json!({"v": 1}), Duration::from_millis(50));
    assert!(cache.get("p").is_some());
    std::thread::sleep(Duration::from_millis(80));
    assert!(cache.get("p").is_none());
}
