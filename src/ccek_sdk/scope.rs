//! CCEK Scope - Kotlin CoroutineScope pattern

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::context::CcekContext;

pub trait CcekScope: Send + Sync {
    fn context(&self) -> &CcekContext;
}

impl<C> CcekScope for C
where
    C: AsRef<CcekContext>,
{
    fn context(&self) -> &CcekContext {
        self.as_ref().as_ref()
    }
}

#[derive(Clone)]
pub struct CcekScopeHandle {
    ctx: Arc<dyn CcekScope>,
}

impl CcekScopeHandle {
    pub fn new(ctx: CcekContext) -> Self {
        Self { ctx: Arc::new(ctx) }
    }
    pub fn context(&self) -> &CcekContext {
        self.ctx.context()
    }
    pub fn child(&self, ctx: CcekContext) -> CcekScopeHandle {
        CcekScopeHandle::new(ctx)
    }
}

impl Default for CcekScopeHandle {
    fn default() -> Self {
        Self::new(CcekContext::new())
    }
}

#[derive(Clone)]
pub struct CcekScopeRef {
    scope: Arc<dyn CcekScope>,
}

impl CcekScopeRef {
    pub fn new(scope: impl CcekScope + 'static) -> Self {
        Self {
            scope: Arc::new(scope),
        }
    }
    pub fn context(&self) -> &CcekContext {
        self.scope.context()
    }
    pub fn narrow(&self, ctx: CcekContext) -> CcekScopeRef {
        CcekScopeRef::new(CcekScopeHandle::new(ctx))
    }
}

impl AsRef<CcekContext> for CcekScopeRef {
    fn as_ref(&self) -> &CcekContext {
        self.context()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccek_sdk::EmptyContext;

    #[test]
    fn test_scope() {
        let scope = CcekScopeRef::new(CcekScopeHandle::default());
        assert!(scope.context().get::<()>().is_none());
    }
}
