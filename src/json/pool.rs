/// Thread-safe object pool using lock-free queues
///
/// This module provides `AtomicPool<T>`, a thread-safe pool for reusing objects
/// across multiple threads without locks. It uses crossbeam's lock-free SegQueue
/// for maximum performance under concurrent access.
///
/// # Architecture
///
/// - Uses `crossbeam::queue::SegQueue` for lock-free push/pop
/// - Supports any type `T: Send + 'static`
/// - Objects are returned to the pool when dropped
/// - Pool size is unbounded but self-limiting via object reuse
///
/// # Thread Safety
///
/// All operations are thread-safe and can be called concurrently without
/// external synchronization. The internal queue uses atomic operations for
/// coordination.
///
/// # Example
///
/// ```rust
/// use literbike::json::pool::AtomicPool;
///
/// let pool = AtomicPool::new();
///
/// // Get an object from the pool (or create new)
/// let obj = pool.get_or_create(|| Vec::new());
///
/// // Use the object
/// obj.push(1);
///
/// // Return to pool explicitly
/// pool.put(obj);
///
/// // Or let it drop and return automatically (if using Pooled<T>)
/// ```
use crossbeam::queue::SegQueue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Thread-safe pool for object reuse
///
/// Uses a lock-free queue for maximum performance under concurrent access.
/// Objects are recycled to avoid allocation overhead.
///
/// # Memory Safety
///
/// - Uses Acquire/Release ordering for proper synchronization
/// - Pool has maximum size to prevent unbounded growth
/// - Counters are updated atomically with queue operations
pub struct AtomicPool<T> {
    queue: Arc<SegQueue<T>>,
    total_created: Arc<AtomicUsize>,
    current_size: Arc<AtomicUsize>,
    max_size: usize,
}

impl<T> AtomicPool<T>
where
    T: Send + 'static,
{
    /// Create a new empty pool with default max size (1000)
    ///
    /// # Example
    ///
    /// ```rust
    /// use literbike::json::pool::AtomicPool;
    ///
    /// let pool: AtomicPool<Vec<u8>> = AtomicPool::new();
    /// ```
    pub fn new() -> Self {
        Self::with_max_size(1000)
    }

    /// Create a new empty pool with specified max size
    ///
    /// # Arguments
    ///
    /// * `max_size` - Maximum number of objects to keep in the pool
    ///
    /// # Example
    ///
    /// ```rust
    /// use literbike::json::pool::AtomicPool;
    ///
    /// let pool: AtomicPool<Vec<u8>> = AtomicPool::with_max_size(500);
    /// ```
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            queue: Arc::new(SegQueue::new()),
            total_created: Arc::new(AtomicUsize::new(0)),
            current_size: Arc::new(AtomicUsize::new(0)),
            max_size,
        }
    }

    /// Get an object from the pool, or create a new one if empty
    ///
    /// This is lock-free and will return immediately with either a reused
    /// object or a newly created one.
    ///
    /// # Arguments
    ///
    /// * `factory` - Function to create a new object if pool is empty
    ///
    /// # Example
    ///
    /// ```rust
    /// use literbike::json::pool::AtomicPool;
    ///
    /// let pool: AtomicPool<Vec<u8>> = AtomicPool::new();
    /// let vec = pool.get_or_create(|| Vec::with_capacity(1024));
    /// ```
    pub fn get_or_create<F>(&self, factory: F) -> T
    where
        F: FnOnce() -> T,
    {
        // Try to pop from pool with proper acquire semantics
        if let Some(obj) = self.queue.pop() {
            // Use Acquire ordering to synchronize with the Release in put()
            self.current_size.fetch_sub(1, Ordering::Acquire);
            return obj;
        }

        // Pool empty, create new object
        let obj = factory();
        // Use Relaxed for total_created since it's just statistics
        self.total_created.fetch_add(1, Ordering::Relaxed);
        obj
    }

    /// Return an object to the pool for reuse
    ///
    /// Objects should be in a clean state before returning.
    /// If the pool is at max capacity, the object will be dropped
    /// to prevent unbounded memory growth.
    ///
    /// # Arguments
    ///
    /// * `obj` - Object to return to the pool
    ///
    /// # Example
    ///
    /// ```rust
    /// use literbike::json::pool::AtomicPool;
    ///
    /// let pool: AtomicPool<Vec<u8>> = AtomicPool::new();
    /// let mut vec = pool.get_or_create(|| Vec::new());
    /// vec.clear(); // Reset to clean state
    /// pool.put(vec);
    /// ```
    pub fn put(&self, obj: T) {
        // Check current size BEFORE pushing to prevent race condition
        let current = self.current_size.load(Ordering::Acquire);

        if current >= self.max_size {
            // Pool at capacity, drop the object to prevent leak
            return;
        }

        // Try to increment size, but check again to prevent race
        match self.current_size.compare_exchange(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Successfully reserved a slot, now push to queue
                self.queue.push(obj);
            }
            Err(_) => {
                // Another thread added an object, check new size
                let new_current = self.current_size.load(Ordering::Acquire);
                if new_current < self.max_size {
                    // Still under capacity, try again
                    self.queue.push(obj);
                    self.current_size.fetch_add(1, Ordering::Release);
                }
                // Otherwise, drop the object
            }
        }
    }

    /// Get the number of objects currently in the pool
    ///
    /// This is an approximate value due to concurrent access.
    pub fn size(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }

    /// Get the total number of objects created (pool size + in-use)
    pub fn total_created(&self) -> usize {
        self.total_created.load(Ordering::Relaxed)
    }

    /// Clear all objects from the pool
    ///
    /// Useful for cleanup or memory reclamation.
    pub fn clear(&self) {
        while let Some(_) = self.queue.pop() {
            // Use Acquire to synchronize with any pending operations
            self.current_size.fetch_sub(1, Ordering::Acquire);
        }
    }
}

impl<T> Clone for AtomicPool<T> {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
            total_created: Arc::clone(&self.total_created),
            current_size: Arc::clone(&self.current_size),
            max_size: self.max_size,
        }
    }
}

impl<T> Default for AtomicPool<T>
where
    T: Send + 'static,
{
    fn default() -> Self {
        Self::with_max_size(1000)
    }
}

/// A wrapper that automatically returns objects to the pool when dropped
///
/// This ensures objects are always returned, even if a panic occurs.
///
/// # Thread Safety
///
/// `Pooled<T>` is `Send` when `T: Send`, allowing it to be transferred
/// across threads safely.
///
/// # Example
///
/// ```rust
/// use literbike::json::pool::{AtomicPool, Pooled};
///
/// let pool: AtomicPool<Vec<u8>> = AtomicPool::new();
/// let mut vec = Pooled::new(pool, || Vec::new());
/// vec.push(1);
/// // vec is automatically returned to pool when dropped
/// ```
pub struct Pooled<T: Send + 'static> {
    obj: Option<T>,
    pool: AtomicPool<T>,
}

impl<T: Send + 'static> Pooled<T> {
    /// Create a new pooled object
    ///
    /// # Arguments
    ///
    /// * `pool` - The pool to use
    /// * `factory` - Function to create object if pool is empty
    pub fn new(pool: AtomicPool<T>, factory: impl FnOnce() -> T) -> Self
    where
        T: Send + 'static,
    {
        let obj = pool.get_or_create(factory);
        Self {
            obj: Some(obj),
            pool,
        }
    }

    /// Get a reference to the inner object
    pub fn get(&self) -> &T {
        self.obj.as_ref().expect("Object not taken")
    }

    /// Get a mutable reference to the inner object
    pub fn get_mut(&mut self) -> &mut T {
        self.obj.as_mut().expect("Object not taken")
    }

    /// Take the object out of the wrapper, preventing automatic return
    ///
    /// You must manually return it to the pool with `pool.put()`.
    pub fn take(mut self) -> T
    where
        T: Send + 'static,
    {
        self.obj.take().expect("Object already taken")
    }

    /// Manually return the object to the pool before drop
    pub fn return_to_pool(mut self)
    where
        T: Send + 'static,
    {
        if let Some(obj) = self.obj.take() {
            self.pool.put(obj);
        }
    }
}

impl<T: Send + 'static> std::ops::Deref for Pooled<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: Send + 'static> std::ops::DerefMut for Pooled<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T> Drop for Pooled<T>
where
    T: Send + 'static,
{
    fn drop(&mut self) {
        if let Some(obj) = self.obj.take() {
            self.pool.put(obj);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_pool_basic() {
        let pool: AtomicPool<Vec<u8>> = AtomicPool::new();

        // Create and return object
        let vec = pool.get_or_create(|| Vec::with_capacity(1024));
        assert_eq!(pool.total_created(), 1);
        pool.put(vec);

        // Should reuse the same object
        let vec2 = pool.get_or_create(|| Vec::with_capacity(1024));
        assert_eq!(pool.total_created(), 1); // No new object created
        assert_eq!(vec2.capacity(), 1024); // Same capacity as original
        pool.put(vec2);

        // Size should be 1
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn test_pool_concurrent() {
        let pool: AtomicPool<Vec<usize>> = Arc::new(AtomicPool::new());
        let mut handles = vec![];

        for i in 0..10 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let mut vec = pool_clone.get_or_create(|| Vec::with_capacity(16));
                    vec.push(i);
                    pool_clone.put(vec);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All objects should be returned
        assert_eq!(pool.size(), pool.total_created());
    }

    #[test]
    fn test_pooled_auto_return() {
        let pool: AtomicPool<Vec<u8>> = AtomicPool::new();

        {
            let mut vec = Pooled::new(pool.clone(), || Vec::with_capacity(1024));
            vec.push(1);
            assert_eq!(vec.len(), 1);
        } // vec is automatically returned here

        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn test_pooled_take() {
        let pool: AtomicPool<Vec<u8>> = AtomicPool::new();

        let mut pooled = Pooled::new(pool.clone(), || Vec::with_capacity(1024));
        pooled.push(1);

        let vec = pooled.take();
        assert_eq!(vec.len(), 1);

        // Must manually return
        pool.put(vec);
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn test_pool_clear() {
        let pool: AtomicPool<Vec<u8>> = AtomicPool::new();

        for _ in 0..10 {
            let vec = pool.get_or_create(|| Vec::new());
            pool.put(vec);
        }

        assert_eq!(pool.size(), 10);
        pool.clear();
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn test_pool_clone() {
        let pool1: AtomicPool<Vec<u8>> = AtomicPool::new();
        let pool2 = pool1.clone();

        let vec = pool1.get_or_create(|| Vec::with_capacity(1024));
        pool1.put(vec);

        // Both clones share the same underlying queue
        assert_eq!(pool2.size(), 1);
    }

    #[test]
    fn test_pool_stress() {
        let pool: Arc<AtomicPool<Vec<usize>>> = Arc::new(AtomicPool::new());
        let mut handles = vec![];

        // Spawn 100 threads, each doing 1000 get/put operations
        for i in 0..100 {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                for j in 0..1000 {
                    let mut vec = pool_clone.get_or_create(|| Vec::with_capacity(16));
                    vec.push(i * 1000 + j);
                    pool_clone.put(vec);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All operations should complete without panics
        assert_eq!(pool.size(), pool.total_created());
    }
}
