//! CCEK Scope - Kotlin CoroutineScope translation

use super::{CoroutineContext, CoroutineScope, EmptyCoroutineContext};
use std::sync::Arc;

pub struct Scope {
    ctx: Arc<dyn CoroutineContext>,
}

impl Scope {
    pub fn new(ctx: impl CoroutineContext + 'static) -> Self {
        Self { ctx: Arc::new(ctx) }
    }
    pub fn context(&self) -> &dyn CoroutineContext {
        &*self.ctx
    }
    pub fn child(&self, ctx: impl CoroutineContext + 'static) -> Scope {
        Scope::new(ctx)
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new(EmptyCoroutineContext)
    }
}

impl CoroutineScope for Scope {
    fn coroutine_context(&self) -> &dyn CoroutineContext {
        &*self.ctx
    }
}

impl AsRef<dyn CoroutineContext> for Scope {
    fn as_ref(&self) -> &dyn CoroutineContext {
        &*self.ctx
    }
}
