use std::time::{Duration, Instant};
use std::collections::HashMap;
use crate::quic_ccek_types::{CcekPolicy};
use crate::rbcursive::{Indexed, Join};

/// Key graph for protocol transition reactor continuations
/// Categorical composition: KeyGraph = Join<StateKey, TransitionMap>
type StateKey = u64;
type TransitionMap = HashMap<StateKey, ProtocolTransition>;
type KeyGraph = Join<StateKey, TransitionMap>;

/// Protocol transition with reactor continuation context
#[derive(Clone, Debug)]
pub struct ProtocolTransition {
    pub from_state: StateKey,
    pub to_state: StateKey, 
    pub transition_guard: fn(&CcekContext) -> bool,
    pub reactor_continuation: fn(&mut CcekContext) -> Result<(), CcekError>,
}

/// CCEK reactor context with key graph navigation
#[derive(Debug)]
pub struct CcekContext {
    pub current_state: StateKey,
    pub last_transition: Instant,
    pub continuation_stack: Vec<StateKey>,
    pub protocol_metadata: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Clone)]
pub enum CcekError {
    InvalidTransition(StateKey, StateKey),
    ReactorContinuationFailed(String),
    KeyGraphCorruption,
}

#[derive(Clone, Debug, Default)]
pub struct QuicCcek {
    pub policy: CcekPolicy,
    pub key_graph: KeyGraph,
    pub context: Option<CcekContext>,
}

impl QuicCcek {
    /// Initialize CCEK with key graph for protocol transitions
    pub fn new_with_key_graph() -> Self {
        let initial_state: StateKey = 0x1000; // Initial QUIC handshake state
        let mut transition_map = HashMap::new();
        
        // Define protocol transition graph for reactor continuations
        transition_map.insert(0x1000, ProtocolTransition {
            from_state: 0x1000,
            to_state: 0x1001, // Handshake -> Connected
            transition_guard: |ctx| ctx.current_state == 0x1000,
            reactor_continuation: |ctx| {
                ctx.current_state = 0x1001;
                ctx.continuation_stack.push(0x1001);
                Ok(())
            },
        });
        
        transition_map.insert(0x1001, ProtocolTransition {
            from_state: 0x1001,
            to_state: 0x1002, // Connected -> Data Transfer
            transition_guard: |ctx| ctx.current_state == 0x1001,
            reactor_continuation: |ctx| {
                ctx.current_state = 0x1002;
                ctx.continuation_stack.push(0x1002);
                Ok(())
            },
        });
        
        let key_graph = KeyGraph(initial_state, transition_map);
        
        Self {
            policy: CcekPolicy::default(),
            key_graph,
            context: Some(CcekContext {
                current_state: initial_state,
                last_transition: Instant::now(),
                continuation_stack: vec![initial_state],
                protocol_metadata: HashMap::new(),
            }),
        }
    }
    
    /// Execute reactor continuation based on key graph protocol transitions
    pub fn execute_reactor_continuation(&mut self, target_state: StateKey) -> Result<(), CcekError> {
        let ctx = self.context.as_mut().ok_or(CcekError::KeyGraphCorruption)?;
        let current = ctx.current_state;
        
        // Look up transition in key graph
        let transition = self.key_graph.1.get(&current)
            .ok_or(CcekError::InvalidTransition(current, target_state))?;
            
        // Validate transition guard
        if !(transition.transition_guard)(ctx) {
            return Err(CcekError::InvalidTransition(current, target_state));
        }
        
        // Execute reactor continuation
        (transition.reactor_continuation)(ctx)
            .map_err(|_| CcekError::ReactorContinuationFailed(format!("Transition {}->{}", current, target_state)))?;
            
        ctx.last_transition = Instant::now();
        Ok(())
    }
    
    /// Key graph navigation with categorical composition
    pub fn navigate_key_graph(&self, from: StateKey, to: StateKey) -> Option<&ProtocolTransition> {
        self.key_graph.1.get(&from).filter(|transition| transition.to_state == to)
    }
    
    pub fn should_emit_cover(&self, last: Instant, now: Instant) -> bool {
        if !self.policy.enable_cover { return false; }
        let elapsed = now.saturating_duration_since(last).as_millis() as u32;
        elapsed > self.policy.cadence.idle_ms
    }
    
    pub fn cover_delay(&self) -> Duration {
        Duration::from_millis(self.policy.cadence.burst_ms as u64)
    }
    
    /// Get current reactor continuation stack for protocol analysis
    pub fn get_continuation_stack(&self) -> Vec<StateKey> {
        self.context.as_ref()
            .map(|ctx| ctx.continuation_stack.clone())
            .unwrap_or_default()
    }
}
