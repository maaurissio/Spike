//! Caché L1 concurrente y efímera para respuestas normalizadas.
#![allow(dead_code)] // Se conecta a los providers de perfil/historial en la siguiente capability.

use std::{sync::Arc, time::Duration};

use moka::sync::Cache;

/// Configuración explícita de una caché L1; no hay persistencia en disco.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct L1CacheSettings {
    pub capacity: u64,
    pub ttl: Duration,
}

impl Default for L1CacheSettings {
    fn default() -> Self {
        Self {
            capacity: 256,
            ttl: Duration::from_secs(60),
        }
    }
}

/// Valores en `Arc`: los consumidores no copian respuestas grandes de perfil o partida.
#[derive(Clone)]
pub struct L1Cache<V> {
    inner: Cache<String, Arc<V>>,
}

impl<V> L1Cache<V>
where
    V: Send + Sync + 'static,
{
    pub fn new(settings: L1CacheSettings) -> Self {
        let inner = Cache::builder()
            .max_capacity(settings.capacity)
            .time_to_live(settings.ttl)
            .build();
        Self { inner }
    }

    pub fn get(&self, key: &str) -> Option<Arc<V>> {
        self.inner.get(key)
    }

    /// Obtiene una respuesta o ejecuta una sola carga compartida por clave.
    pub fn get_or_insert_with(&self, key: impl Into<String>, load: impl FnOnce() -> V) -> Arc<V> {
        self.inner.get_with(key.into(), || Arc::new(load()))
    }

    pub fn insert(&self, key: impl Into<String>, value: V) {
        self.inner.insert(key.into(), Arc::new(value));
    }

    pub fn invalidate(&self, key: &str) {
        self.inner.invalidate(key);
    }

    pub fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn reuses_loaded_value_for_the_same_key() {
        let cache = L1Cache::new(L1CacheSettings::default());
        let loads = AtomicUsize::new(0);

        let first = cache.get_or_insert_with("profile:me", || {
            loads.fetch_add(1, Ordering::Relaxed);
            "first".to_owned()
        });
        let second = cache.get_or_insert_with("profile:me", || {
            loads.fetch_add(1, Ordering::Relaxed);
            "second".to_owned()
        });

        assert_eq!(&*first, "first");
        assert_eq!(&*second, "first");
        assert_eq!(loads.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn invalidation_removes_cached_value() {
        let cache = L1Cache::new(L1CacheSettings::default());
        cache.insert("history:me", vec!["match-1"]);
        assert_eq!(cache.get("history:me").as_deref(), Some(&vec!["match-1"]));

        cache.invalidate("history:me");
        cache.inner.run_pending_tasks();
        assert!(cache.get("history:me").is_none());
    }
}
