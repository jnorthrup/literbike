//! Session Islands - Isolated execution contexts for structured concurrency
//!
//! Session islands provide isolation boundaries for coroutines, similar to
//! Kotlin's structured concurrency model. Each island has its own context
//! elements and can have parent/child relationships for hierarchical shutdown.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

/// Context element key for CCEK pattern
pub trait ContextElementKey: Send + Sync + 'static {
    type Element: ContextElement;
}

/// Context element trait
pub trait ContextElement: Send + Sync + Any {
    fn clone_element(&self) -> Box<dyn ContextElement>;
    fn as_any(&self) -> &dyn Any;
}

/// Channel registry for protocol-specific channels
#[derive(Debug, Clone)]
pub struct ChannelRegistry {
    channels: Arc<RwLock<HashMap<String, ChannelHandle>>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn insert(&self, name: String, handle: ChannelHandle) {
        let mut channels = self.channels.write().unwrap();
        channels.insert(name, handle);
    }

    pub fn get(&self, name: &str) -> Option<ChannelHandle> {
        let channels = self.channels.read().unwrap();
        channels.get(name).cloned()
    }

    pub fn remove(&self, name: &str) {
        let mut channels = self.channels.write().unwrap();
        channels.remove(name);
    }

    pub fn clear(&self) {
        let mut channels = self.channels.write().unwrap();
        channels.clear();
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Channel handle for inter-island communication
#[derive(Debug, Clone)]
pub struct ChannelHandle {
    pub name: String,
    pub protocol: String,
    sender: Arc<dyn Send + Sync>,
}

impl ChannelHandle {
    pub fn new(name: String, protocol: String) -> Self {
        Self {
            name,
            protocol,
            sender: Arc::new(()),
        }
    }
}

/// Session island - isolated execution context
pub struct SessionIsland {
    id: u64,
    name: String,
    elements: Arc<RwLock<HashMap<TypeId, Box<dyn ContextElement>>>>,
    channels: ChannelRegistry,
    parent: Option<Arc<SessionIsland>>,
    children: Arc<RwLock<Vec<Arc<SessionIsland>>>>,
    cancelled: Arc<RwLock<bool>>,
}

impl SessionIsland {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            id,
            name,
            elements: Arc::new(RwLock::new(HashMap::new())),
            channels: ChannelRegistry::new(),
            parent: None,
            children: Arc::new(RwLock::new(Vec::new())),
            cancelled: Arc::new(RwLock::new(false)),
        }
    }

    pub fn with_parent(parent: Arc<SessionIsland>, id: u64, name: String) -> Self {
        let island = Self::new(id, name);
        island.parent = Some(parent.clone());

        let mut children = parent.children.write().unwrap();
        children.push(Arc::clone(&island));

        island
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.read().unwrap()
    }

    pub fn cancel(&self) {
        *self.cancelled.write().unwrap() = true;

        let children = self.children.read().unwrap();
        for child in children.iter() {
            child.cancel();
        }
    }

    pub fn cancel_children(&self) {
        let children = self.children.read().unwrap();
        for child in children.iter() {
            child.cancel();
        }
    }

    pub fn channels(&self) -> &ChannelRegistry {
        &self.channels
    }

    pub fn parent(&self) -> Option<Arc<SessionIsland>> {
        self.parent.clone()
    }

    pub fn children(&self) -> Vec<Arc<SessionIsland>> {
        self.children.read().unwrap().clone()
    }

    pub fn attach_element<E: ContextElement + 'static>(&self, element: E) {
        let type_id = TypeId::of::<E>();
        let mut elements = self.elements.write().unwrap();
        elements.insert(type_id, Box::new(element));
    }

    pub fn get_element<E: ContextElement + 'static>(&self) -> Option<E> {
        let type_id = TypeId::of::<E>();
        let elements = self.elements.read().unwrap();
        elements
            .get(&type_id)
            .and_then(|e| e.as_any().downcast_ref::<E>().cloned())
    }

    pub fn remove_element<E: ContextElement + 'static>(&self) {
        let type_id = TypeId::of::<E>();
        let mut elements = self.elements.write().unwrap();
        elements.remove(&type_id);
    }
}

impl Debug for SessionIsland {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionIsland")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("parent", &self.parent.is_some())
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Context element for session reference
#[derive(Debug, Clone)]
pub struct SessionElement {
    pub session: Arc<SessionIsland>,
}

impl ContextElement for SessionElement {
    fn clone_element(&self) -> Box<dyn ContextElement> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Context element for cancellation
#[derive(Debug, Clone)]
pub struct CancellationElement {
    pub token: CancellationToken,
}

impl ContextElement for CancellationElement {
    fn clone_element(&self) -> Box<dyn ContextElement> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Cancellation token for structured cancellation
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<RwLock<bool>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(RwLock::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.read().unwrap()
    }

    pub fn cancel(&self) {
        *self.cancelled.write().unwrap() = true;
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Key graph for protocol state machines
pub type KeyGraph = (StateKey, TransitionMap);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateKey(pub String);

#[derive(Debug, Clone)]
pub struct TransitionMap {
    transitions: HashMap<StateKey, Vec<ProtocolTransition>>,
}

impl TransitionMap {
    pub fn new() -> Self {
        Self {
            transitions: HashMap::new(),
        }
    }

    pub fn add_transition(&mut self, from: StateKey, transition: ProtocolTransition) {
        self.transitions.entry(from).or_default().push(transition);
    }

    pub fn get_transitions(&self, from: &StateKey) -> Vec<ProtocolTransition> {
        self.transitions.get(from).cloned().unwrap_or_default()
    }
}

impl Default for TransitionMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Protocol transition for reactor continuations
#[derive(Debug, Clone)]
pub struct ProtocolTransition {
    pub from_state: StateKey,
    pub to_state: StateKey,
    pub guard: Arc<dyn Fn(&CcekContext) -> bool + Send + Sync>,
    pub continuation: Arc<dyn Fn(&mut CcekContext) -> Result<(), CcekError> + Send + Sync>,
}

impl ProtocolTransition {
    pub fn new<F, G, C>(from: StateKey, to: StateKey, guard: F, continuation: C) -> Self
    where
        F: Fn(&CcekContext) -> bool + Send + Sync + 'static,
        C: Fn(&mut CcekContext) -> Result<(), CcekError> + Send + Sync + 'static,
    {
        Self {
            from_state: from,
            to_state: to,
            guard: Arc::new(guard),
            continuation: Arc::new(continuation),
        }
    }
}

/// CCEK context for key graph navigation
#[derive(Debug)]
pub struct CcekContext {
    current_state: StateKey,
    last_transition: std::time::Instant,
    continuation_stack: Vec<StateKey>,
    protocol_metadata: HashMap<String, Vec<u8>>,
}

impl CcekContext {
    pub fn new(initial_state: StateKey) -> Self {
        Self {
            current_state: initial_state,
            last_transition: std::time::Instant::now(),
            continuation_stack: Vec::new(),
            protocol_metadata: HashMap::new(),
        }
    }

    pub fn current_state(&self) -> &StateKey {
        &self.current_state
    }

    pub fn transition_to(&mut self, state: StateKey) {
        self.continuation_stack.push(self.current_state.clone());
        self.current_state = state;
        self.last_transition = std::time::Instant::now();
    }

    pub fn push_metadata(&mut self, key: String, value: Vec<u8>) {
        self.protocol_metadata.insert(key, value);
    }

    pub fn get_metadata(&self, key: &str) -> Option<&Vec<u8>> {
        self.protocol_metadata.get(key)
    }
}

#[derive(Debug)]
pub enum CcekError {
    TransitionFailed(String),
    InvalidState(String),
    Cancelled,
}

impl std::fmt::Display for CcekError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransitionFailed(msg) => write!(f, "Transition failed: {}", msg),
            Self::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl std::error::Error for CcekError {}

/// Execute a key graph with the given context
pub fn execute_key_graph(graph: &KeyGraph, ctx: &mut CcekContext) -> Result<(), CtekError> {
    let (initial_state, transitions) = graph;

    loop {
        let current = ctx.current_state();
        let trans = transitions.get_transitions(current);

        if trans.is_empty() {
            break;
        }

        let mut made_transition = false;
        for t in trans {
            if (t.guard)(ctx) {
                t.continuation(ctx)?;
                ctx.transition_to(t.to_state.clone());
                made_transition = true;
                break;
            }
        }

        if !made_transition {
            break;
        }
    }

    Ok(())
}

type CtekError = CcekError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_island_creation() {
        let island = SessionIsland::new(1, "test".to_string());
        assert_eq!(island.id(), 1);
        assert_eq!(island.name(), "test");
        assert!(!island.is_cancelled());
    }

    #[test]
    fn test_session_island_cancel() {
        let island = SessionIsland::new(1, "test".to_string());
        island.cancel();
        assert!(island.is_cancelled());
    }

    #[test]
    fn test_session_island_parent_child() {
        let parent = Arc::new(SessionIsland::new(1, "parent".to_string()));
        let child = Arc::new(SessionIsland::with_parent(
            parent.clone(),
            2,
            "child".to_string(),
        ));

        assert_eq!(child.parent().map(|p| p.id()), Some(1));
        assert!(parent.children().contains(&child));
    }

    #[test]
    fn test_channel_registry() {
        let registry = ChannelRegistry::new();
        let handle = ChannelHandle::new("test".to_string(), "htx".to_string());

        registry.insert("test".to_string(), handle.clone());
        assert_eq!(
            registry.get("test").map(|h| h.name),
            Some("test".to_string())
        );

        registry.remove("test");
        assert!(registry.get("test").is_none());
    }

    #[test]
    fn test_ccek_context() {
        let mut ctx = CcekContext::new(StateKey("start".to_string()));
        assert_eq!(ctx.current_state().0, "start");

        ctx.transition_to(StateKey("middle".to_string()));
        assert_eq!(ctx.current_state().0, "middle");

        ctx.push_metadata("key".to_string(), vec![1, 2, 3]);
        assert_eq!(ctx.get_metadata("key"), Some(&vec![1, 2, 3]));
    }

    #[test]
    fn test_key_graph_execution() {
        let mut transitions = TransitionMap::new();
        transitions.add_transition(
            StateKey("start".to_string()),
            ProtocolTransition::new(
                StateKey("start".to_string()),
                StateKey("middle".to_string()),
                |_| true,
                |_| Ok(()),
            ),
        );
        transitions.add_transition(
            StateKey("middle".to_string()),
            ProtocolTransition::new(
                StateKey("middle".to_string()),
                StateKey("end".to_string()),
                |_| true,
                |_| Ok(()),
            ),
        );

        let graph = (StateKey("start".to_string()), transitions);
        let mut ctx = CcekContext::new(StateKey("start".to_string()));

        execute_key_graph(&graph, &mut ctx).unwrap();
        assert_eq!(ctx.current_state().0, "end");
    }
}
