//! CCEK - Kotlin CoroutineContext pattern
//!
//! Exact Kotlin mirror with TypeId as key type.

use std::any::{Any, TypeId};

pub trait CcekKey: 'static {
    type Element: CcekElement;
}

pub trait CcekElement: Send + Sync + 'static {
    fn key(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Clone, Default)]
pub struct CcekContext;

impl CcekContext {
    pub fn new() -> Self {
        Self
    }

    pub fn get<E: CcekElement + 'static>(&self) -> Option<&E> {
        None
    }

    pub fn minus_key(&self, _key: TypeId) -> Self {
        self.clone()
    }
}

impl std::ops::Add for CcekContext {
    type Output = Self;
    fn add(self, _rhs: Self) -> Self::Output {
        self
    }
}

#[derive(Clone, Default)]
pub struct EmptyContext;

impl CcekElement for EmptyContext {
    fn key(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}
