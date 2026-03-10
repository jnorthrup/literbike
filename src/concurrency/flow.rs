//! Simplified Flow module
//! Flow-based reactive streams with backpressure


/// Flow - a cold asynchronous stream of values
pub struct Flow<T: Send + 'static> {
    values: Vec<T>,
}

impl<T: Send + 'static> Flow<T> {
    /// Create a flow from an iterator
    pub fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        Self {
            values: iter.into_iter().collect(),
        }
    }
    
    /// Create a flow from a single value
    pub fn just(value: T) -> Self {
        Self { values: vec![value] }
    }
    
    /// Create an empty flow
    pub fn empty() -> Self {
        Self { values: vec![] }
    }
    
    /// Collect values into a Vec
    pub fn to_vec(self) -> Vec<T> {
        self.values
    }
    
    /// Map operator - transform each element
    pub fn map<F, U>(self, f: F) -> Flow<U>
    where
        F: Fn(T) -> U,
        U: Send + 'static,
    {
        Flow {
            values: self.values.into_iter().map(f).collect(),
        }
    }
    
    /// Filter operator - keep only elements that match predicate
    pub fn filter<F>(self, predicate: F) -> Flow<T>
    where
        F: Fn(&T) -> bool,
    {
        Flow {
            values: self.values.into_iter().filter(predicate).collect(),
        }
    }
    
    /// Take operator - take first n elements
    pub fn take(self, n: usize) -> Flow<T> {
        Flow {
            values: self.values.into_iter().take(n).collect(),
        }
    }
}

/// Flow builder for complex flow construction
pub struct FlowBuilder<T: Send + 'static> {
    values: Vec<T>,
}

impl<T: Send + 'static> FlowBuilder<T> {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
    
    pub fn add(mut self, value: T) -> Self {
        self.values.push(value);
        self
    }
    
    pub fn build(self) -> Flow<T> {
        Flow::from_iter(self.values)
    }
}

impl<T: Send + 'static> Default for FlowBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Flow operator trait for extensibility
pub trait FlowOperator<T: Send + 'static> {
    fn apply(&self, flow: Flow<T>) -> Flow<T>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_from_iter() {
        let flow = Flow::from_iter(vec![1, 2, 3, 4, 5]);
        let values = flow.to_vec();
        assert_eq!(values, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_flow_map() {
        let flow = Flow::from_iter(vec![1, 2, 3]).map(|x| x * 2);
        let values = flow.to_vec();
        assert_eq!(values, vec![2, 4, 6]);
    }

    #[test]
    fn test_flow_filter() {
        let flow = Flow::from_iter(vec![1, 2, 3, 4, 5]).filter(|x| x % 2 == 0);
        let values = flow.to_vec();
        assert_eq!(values, vec![2, 4]);
    }

    #[test]
    fn test_flow_take() {
        let flow = Flow::from_iter(vec![1, 2, 3, 4, 5]).take(3);
        let values = flow.to_vec();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn test_flow_chain() {
        let flow = Flow::from_iter(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
            .filter(|x| x % 2 == 0)
            .map(|x| x * 10)
            .take(3);
        let values = flow.to_vec();
        assert_eq!(values, vec![20, 40, 60]);
    }
}
