// Event handler abstraction for reactor dispatch

use std::sync::Arc;
use parking_lot::Mutex;

pub trait Handler: Send + Sync {
    fn handle_event(&self, event: &[u8]) -> Result<(), String>;
    fn shutdown(&self) -> Result<(), String> { Ok(()) }
}

pub struct HandlerRegistry {
    handlers: Arc<Mutex<Vec<Arc<dyn Handler>>>>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self { handlers: Arc::new(Mutex::new(Vec::new())) }
    }
    pub fn register(&self, handler: Arc<dyn Handler>) {
        self.handlers.lock().push(handler);
    }
    pub fn dispatch(&self, event: &[u8]) -> Result<(), String> {
        for h in self.handlers.lock().iter() {
            h.handle_event(event)?;
        }
        Ok(())
    }
    pub fn shutdown_all(&self) -> Result<(), String> {
        for h in self.handlers.lock().iter() {
            h.shutdown()?;
        }
        Ok(())
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Noop;
    impl Handler for Noop {
        fn handle_event(&self, _: &[u8]) -> Result<(), String> { Ok(()) }
    }
    #[test]
    fn dispatch_noop() {
        let r = HandlerRegistry::new();
        r.register(Arc::new(Noop));
        assert!(r.dispatch(b"ping").is_ok());
    }
}
