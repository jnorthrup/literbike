//! CCEK Scope - Implicit CoroutineContext like Kotlin
//!
//! Kotlin: `suspend fun` has implicit CoroutineContext via `currentCoroutineContext()`
//! Rust: We use a Scope trait to provide context implicitly.
//!
//! ## Sequential Composition (no dispatch tables)
//!
//! ```rust
//! // Both Kotlin and Rust are SEQUENTIAL by default
//!
//! // Kotlin
//! suspend fun pipeline(ctx: CoroutineContext) {
//!     val data = source()
//!     val processed = transform(ctx, data)
//!     sink(processed)
//! }
//!
//! // Rust (same sequential pattern)
//! async fn pipeline(ctx: impl CcekScope) {
//!     let data = source().await;
//!     let processed = ctx.transform(data).await;
//!     sink(processed).await;
//! }
//! ```

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
        Self {
            ctx: Arc::new(ctx),
        }
    }

    pub fn scope<F, Fut>(&self, f: F) -> impl Future<Output = ()> + Send
    where
        F: Fn(CcekScopeRef) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let ctx = self.ctx.clone();
        async move {
            let scope = CcekScopeRef(ctx);
            f(scope).await;
        }
    }

    pub async fn run<R, F, Fut>(&self, f: F) -> R
    where
        F: Fn(CcekScopeRef) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let ctx = self.ctx.clone();
        let scope = CcekScopeRef(ctx);
        f(scope).await
    }
}

#[derive(Clone)]
pub struct CcekScopeRef(Arc<dyn CcekScope>);

impl CcekScopeRef {
    pub fn context(&self) -> &CcekContext {
        self.0.context()
    }

    pub fn get<E: CcekElement>(&self) -> Option<&E> {
        self.0.context().get::<E>()
    }

    pub async fn with<R, F, Fut>(&self, element: impl CcekElementAdd, f: F) -> R
    where
        F: Fn(CcekScopeRef) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
    {
        let ctx = self.0.context().clone().with(element);
        let scope = CcekScopeRef(Arc::new(ctx));
        f(scope).await
    }
}

impl AsRef<CcekContext> for CcekScopeRef {
    fn as_ref(&self) -> &CcekContext {
        self.0.context()
    }
}

pub trait CcekElementAdd: Send + Sync + 'static {
    fn add_to(self, ctx: CcekContext) -> CcekContext;
}

impl<T: Send + Sync + 'static> CcekElementAdd for T
where
    T: Clone,
{
    fn add_to(self, mut ctx: CcekContext) -> CcekContext {
        ctx = ctx.with(self);
        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccek_sdk::context::EmptyContext;

    #[test]
    fn test_scope_context() {
        let ctx = EmptyContext;
        let scope = CcekScopeRef(Arc::new(ctx));
        assert!(scope.context().is_empty());
    }
}
