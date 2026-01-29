/// Ring buffer cache for rendered Canvas objects.
///
/// This cache stores rendered content keyed by row index (usize).
/// When capacity is exceeded, it evicts the oldest entries in FIFO order.
use std::collections::BTreeMap;

/// A ring buffer cache that evicts oldest entries when capacity is exceeded.
///
/// # Type Parameters
/// * `T` - The cached value type (e.g., `Canvas`).
///
/// # Example
/// ```rust
/// use render::cache::RingBufferCache;
///
/// let mut cache: RingBufferCache<u32> = RingBufferCache::new(3);
/// cache.insert(0, 10);
/// cache.insert(1, 20);
/// assert_eq!(cache.get(&0), Some(&10));
/// ```
#[derive(Debug, Clone)]
pub struct RingBufferCache<T> {
    /// Storage keyed by row index
    storage: BTreeMap<usize, T>,
    /// Maximum number of entries
    capacity: usize,
    /// Ordered list of keys for FIFO eviction
    keys: Vec<usize>,
}

impl<T> RingBufferCache<T> {
    /// Creates a new ring buffer cache with the given capacity.
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of entries to store
    ///
    /// # Panics
    /// Panics if capacity is zero.
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// assert_eq!(cache.capacity(), 10);
    /// ```
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "cache capacity must be greater than zero");
        Self {
            storage: BTreeMap::new(),
            capacity,
            keys: Vec::with_capacity(capacity),
        }
    }

    /// Returns the maximum capacity of the cache.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the current number of cached entries.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns true if the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Retrieves a cached value by key.
    ///
    /// # Arguments
    /// * `key` - The row index to look up
    ///
    /// # Returns
    /// * `Some(&T)` if the key exists in the cache
    /// * `None` if the key is not present
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// cache.insert(5, 42);
    /// assert_eq!(cache.get(&5), Some(&42));
    /// assert_eq!(cache.get(&10), None);
    /// ```
    pub fn get(&self, key: &usize) -> Option<&T> {
        self.storage.get(key)
    }

    /// Inserts or updates a value in the cache.
    ///
    /// If the key already exists, its value is updated.
    /// If the key is new and the cache is at capacity, the oldest entry is evicted.
    ///
    /// # Arguments
    /// * `key` - The row index to store
    /// * `value` - The value to cache
    ///
    /// # Returns
    /// * `Some(T)` - The old value if the key existed
    /// * `None` - If the key was newly inserted
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(2);
    /// cache.insert(0, 10);
    /// cache.insert(1, 20);
    /// cache.insert(2, 30); // evicts key 0
    /// assert_eq!(cache.get(&0), None);
    /// assert_eq!(cache.get(&1), Some(&20));
    /// assert_eq!(cache.get(&2), Some(&30));
    /// ```
    pub fn insert(&mut self, key: usize, value: T) -> Option<T> {
        let is_new_key = !self.storage.contains_key(&key);

        if is_new_key {
            // Evict oldest entry if at capacity
            if self.keys.len() == self.capacity {
                if let Some(oldest_key) = self.keys.first() {
                    self.storage.remove(oldest_key);
                    self.keys.remove(0);
                }
            }
            // Track this key for eviction
            self.keys.push(key);
        }

        self.storage.insert(key, value)
    }

    /// Removes all entries from the cache.
    ///
    /// This preserves the capacity but clears all stored data.
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// cache.insert(0, 10);
    /// cache.insert(1, 20);
    /// cache.clear();
    /// assert_eq!(cache.len(), 0);
    /// assert_eq!(cache.capacity(), 10);
    /// ```
    pub fn clear(&mut self) {
        self.storage.clear();
        self.keys.clear();
    }

    /// Invalidates all cached entries (alias for clear).
    ///
    /// This method is equivalent to `clear()` but provides semantic clarity
    /// for cache invalidation use cases.
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// cache.insert(0, 10);
    /// cache.invalidate_all();
    /// assert_eq!(cache.len(), 0);
    /// ```
    pub fn invalidate_all(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cache() {
        let cache: RingBufferCache<u32> = RingBufferCache::new(5);
        assert_eq!(cache.capacity(), 5);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    #[should_panic(expected = "cache capacity must be greater than zero")]
    fn test_zero_capacity_panics() {
        let _cache: RingBufferCache<u32> = RingBufferCache::new(0);
    }

    #[test]
    fn test_insert_and_get() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        assert_eq!(cache.insert(0, 10), None);
        assert_eq!(cache.get(&0), Some(&10));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_update_existing_key() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        cache.insert(0, 10);
        assert_eq!(cache.insert(0, 20), Some(10));
        assert_eq!(cache.get(&0), Some(&20));
        assert_eq!(cache.len(), 1); // Length unchanged
    }

    #[test]
    fn test_fifo_eviction() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(2);
        cache.insert(0, 10);
        cache.insert(1, 20);
        cache.insert(2, 30); // Should evict key 0

        assert_eq!(cache.get(&0), None);
        assert_eq!(cache.get(&1), Some(&20));
        assert_eq!(cache.get(&2), Some(&30));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        cache.insert(0, 10);
        cache.insert(1, 20);
        cache.clear();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.capacity(), 5); // Capacity preserved
    }

    #[test]
    fn test_invalidate_all() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        cache.insert(0, 10);
        cache.invalidate_all();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_multiple_evictions() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(2);
        cache.insert(0, 10);
        cache.insert(1, 20);
        cache.insert(2, 30); // Evicts 0
        cache.insert(3, 40); // Evicts 1
        cache.insert(4, 50); // Evicts 2

        assert_eq!(cache.get(&0), None);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&40));
        assert_eq!(cache.get(&4), Some(&50));
    }

    #[test]
    fn test_update_preserves_order() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(3);
        cache.insert(0, 10);
        cache.insert(1, 20);
        cache.insert(2, 30);
        cache.insert(1, 200); // Update key 1

        // Insert new entry - should evict key 0 (oldest insertion)
        cache.insert(3, 40);

        assert_eq!(cache.get(&0), None);
        assert_eq!(cache.get(&1), Some(&200));
        assert_eq!(cache.get(&2), Some(&30));
        assert_eq!(cache.get(&3), Some(&40));
    }

    #[test]
    fn test_sparse_keys() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(3);
        cache.insert(100, 10);
        cache.insert(200, 20);
        cache.insert(300, 30);

        assert_eq!(cache.get(&100), Some(&10));
        assert_eq!(cache.get(&200), Some(&20));
        assert_eq!(cache.get(&300), Some(&30));
    }
}
