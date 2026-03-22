//! CCEK Scope - Kotlin CoroutineScope pattern
//!
//! Scopes narrow future scopes - structured concurrency.
//!
//! ```kotlin
//! coroutineScope {  // narrower than parent
//!     launch { ... }
//!     withContext(Dispatchers.IO) {  // even narrower
//!         ...
//!     }
//! }
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::context::{CcekContext, CcekElement, EmptyContext};

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

    pub async fn with_context<C2, F, Fut>(&self, ctx: C2, f: F) -> Fut::Output
    where
        C2: Into<CcekContext>,
        F: FnOnce(CcekScopeRef) -> Fut,
        Fut: Future,
    {
        let child_scope = self.child(ctx.into());
        let scope_ref = CcekScopeRef::new(child_scope);
        f(scope_ref).await
    }

    pub fn scope<F, Fut>(&self, f: F) -> impl Future<Output = ()> + Send
    where
        F: FnOnce(CcekScopeRef) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    {
        let scope_ref = CcekScopeRef::new(self.clone());
        async move { f(scope_ref).await }
    }
}

impl Default for CcekScopeHandle {
    fn default() -> Self {
        Self::new(EmptyContext)
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

    pub fn child(&self, ctx: CcekContext) -> CcekScopeRef {
        CcekScopeRef::new(CcekScopeHandle::new(ctx))
    }

    pub fn narrow<C2>(&self, ctx: C2) -> CcekScopeRef
    where
        C2: Into<CcekContext>,
    {
        self.child(ctx.into())
    }

    pub async fn with<C2, F, Fut>(&self, ctx: C2, f: F) -> Fut::Output
    where
        C2: Into<CcekContext>,
        F: FnOnce(CcekScopeRef) -> Fut,
        Fut: Future,
    {
        let child = self.narrow(ctx);
        f(child).await
    }
}

impl AsRef<CcekContext> for CcekScopeRef {
    fn as_ref(&self) -> &CcekContext {
        self.context()
    }
}

impl AsRef<dyn CcekScope> for CcekScopeRef {
    fn as_ref(&self) -> &(dyn CcekScope + 'static) {
        &*self.scope
    }
}

pub trait ScopeExt: CcekScope + Sized {
    fn get<E: CcekElement>(&self) -> Option<&E> {
        self.context().get::<E>()
    }
}

impl<S: CcekScope> ScopeExt for S {}

pub struct CcekLocal<T: Send + 'static> {
    value: std::cell::RefCell<Option<T>>,
}

impl<T: Send + 'static> CcekLocal<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: std::cell::RefCell::new(Some(value)),
        }
    }

    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.value.borrow().clone()
    }

    pub fn set(&self, value: T) {
        *self.value.borrow_mut() = Some(value);
    }
}

impl<T: Send + 'static> Default for CcekLocal<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccek_sdk::{HtxElement, HtxKey};

    #[test]
    fn test_scope_narrowing() {
        let parent = CcekScopeHandle::new(EmptyContext);
        assert!(parent.context().is_empty());

        let child_ctx = CcekContext::new().with(HtxElement::new());
        let child = parent.child(child_ctx);
        assert!(child.context().get::<HtxElement>().is_some());
    }

    #[test]
    fn test_scope_ref_narrowing() {
        let parent = CcekScopeRef::new(CcekScopeHandle::default());
        let child = parent.narrow(CcekContext::new().with(HtxElement::new()));
        assert!(child.context().get::<HtxElement>().is_some());
    }

    #[test]
    fn test_scope_context_get() {
        let ctx = CcekContext::new().with(HtxElement::new());
        let scope = CcekScopeHandle::new(ctx);
        let htx = scope.get::<HtxElement>();
        assert!(htx.is_some());
    }
}
