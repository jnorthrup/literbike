// Reactor execution context (port of Kotlin CoroutineContext semantics)

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Clone)]
pub struct ReactorContext {
    state: Arc<RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>>,
}

impl ReactorContext {
    pub fn new() -> Self {
        Self { state: Arc::new(RwLock::new(HashMap::new())) }
    }
    pub fn set<T: Any + Send + Sync + 'static>(&self, key: impl Into<String>, value: T) {
        self.state.write().insert(key.into(), Arc::new(value));
    }
    pub fn get<T: Any + Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        self.state.read().get(key)?.clone().downcast::<T>().ok()
    }
    pub fn remove(&self, key: &str) {
        self.state.write().remove(key);
    }
    pub fn clear(&self) {
        self.state.write().clear();
    }
}

impl Default for ReactorContext {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_and_get() {
        let ctx = ReactorContext::new();
        ctx.set("n", 42i32);
        assert_eq!(*ctx.get::<i32>("n").unwrap(), 42);
    }
}
