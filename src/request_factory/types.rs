use std::marker::PhantomData;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Type-safe entity identifier wrapping a CouchDB `_id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId<T> {
    pub id: String,
    #[serde(skip)]
    _marker: PhantomData<T>,
}

impl<T> EntityId<T> {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), _marker: PhantomData }
    }
}

/// Maps to CouchDB `_rev` — optimistic concurrency token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision(pub String);

impl Revision {
    pub fn new(rev: impl Into<String>) -> Self {
        Self(rev.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Marker trait for entities that have identity (id + revision).
pub trait EntityProxy {
    fn entity_id(&self) -> &str;
    fn revision(&self) -> Option<&str>;
}

/// Marker trait for value objects with no independent identity.
pub trait ValueProxy {}

/// A single operation in a batched request context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Find {
        entity_type: String,
        id: String,
    },
    Persist {
        entity_type: String,
        id: String,
        rev: Option<String>,
        payload: Value,
    },
    Delete {
        entity_type: String,
        id: String,
        rev: String,
    },
}

/// Accumulates operations before firing them as a batch.
#[derive(Debug, Default)]
pub struct RequestContext {
    pub operations: Vec<Operation>,
}

impl RequestContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn find(&mut self, entity_type: impl Into<String>, id: impl Into<String>) {
        self.operations.push(Operation::Find {
            entity_type: entity_type.into(),
            id: id.into(),
        });
    }

    pub fn persist(
        &mut self,
        entity_type: impl Into<String>,
        id: impl Into<String>,
        rev: Option<String>,
        payload: Value,
    ) {
        self.operations.push(Operation::Persist {
            entity_type: entity_type.into(),
            id: id.into(),
            rev,
            payload,
        });
    }

    pub fn delete(
        &mut self,
        entity_type: impl Into<String>,
        id: impl Into<String>,
        rev: impl Into<String>,
    ) {
        self.operations.push(Operation::Delete {
            entity_type: entity_type.into(),
            id: id.into(),
            rev: rev.into(),
        });
    }
}
