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
    /// Number of cache hits
    hits: usize,
    /// Number of cache misses
    misses: usize,
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
            hits: 0,
            misses: 0,
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
    pub fn get(&mut self, key: &usize) -> Option<&T> {
        if self.storage.contains_key(key) {
            self.hits += 1;
            self.storage.get(key)
        } else {
            self.misses += 1;
            None
        }
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
        self.hits = 0;
        self.misses = 0;
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

    /// Invalidates a specific cache entry by key.
    ///
    /// This method removes a single entry from the cache if it exists.
    ///
    /// # Arguments
    /// * `key` - The row index to invalidate
    ///
    /// # Returns
    /// * `Some(T)` - The value if the key existed
    /// * `None` - If the key was not present
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// cache.insert(0, 10);
    /// cache.insert(1, 20);
    /// let removed = cache.invalidate(0);
    /// assert_eq!(removed, Some(10));
    /// assert_eq!(cache.get(&0), None);
    /// assert_eq!(cache.get(&1), Some(&20));
    /// ```
    pub fn invalidate(&mut self, key: usize) -> Option<T> {
        // Remove from keys vector if present
        if let Some(pos) = self.keys.iter().position(|&k| k == key) {
            self.keys.remove(pos);
        }
        // Remove from storage and return the value
        self.storage.remove(&key)
    }

    /// Returns the number of cache hits.
    ///
    /// A hit occurs when `get()` is called with a key that exists in the cache.
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// cache.insert(0, 10);
    /// cache.get(&0); // hit
    /// cache.get(&1); // miss
    /// assert_eq!(cache.get_hit_count(), 1);
    /// ```
    pub fn get_hit_count(&self) -> usize {
        self.hits
    }

    /// Returns the number of cache misses.
    ///
    /// A miss occurs when `get()` is called with a key that does not exist in the cache.
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// cache.insert(0, 10);
    /// cache.get(&0); // hit
    /// cache.get(&1); // miss
    /// assert_eq!(cache.get_miss_count(), 1);
    /// ```
    pub fn get_miss_count(&self) -> usize {
        self.misses
    }

    /// Returns the cache hit rate as a float between 0.0 and 1.0.
    ///
    /// The hit rate is calculated as: `hits / (hits + misses)`.
    /// Returns 0.0 if there have been no cache accesses.
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// cache.insert(0, 10);
    /// cache.get(&0); // hit
    /// cache.get(&1); // miss
    /// assert!((cache.get_hit_rate() - 0.5).abs() < 0.001);
    /// ```
    pub fn get_hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }

    /// Resets hit and miss statistics to zero.
    ///
    /// This does not affect the cached data, only the statistics.
    ///
    /// # Example
    /// ```rust
    /// use render::cache::RingBufferCache;
    ///
    /// let mut cache: RingBufferCache<u32> = RingBufferCache::new(10);
    /// cache.insert(0, 10);
    /// cache.get(&0); // hit
    /// cache.get(&1); // miss
    /// cache.reset_stats();
    /// assert_eq!(cache.get_hit_count(), 0);
    /// assert_eq!(cache.get_miss_count(), 0);
    /// assert_eq!(cache.get_hit_rate(), 0.0);
    /// ```
    pub fn reset_stats(&mut self) {
        self.hits = 0;
        self.misses = 0;
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
        assert_eq!(cache.get_hit_count(), 1);
        assert_eq!(cache.get_miss_count(), 0);
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
        assert_eq!(cache.get_hit_count(), 2);
        assert_eq!(cache.get_miss_count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        cache.insert(0, 10);
        cache.insert(1, 20);
        cache.get(&0); // Generate some stats
        cache.get(&5); // Generate a miss
        cache.clear();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.capacity(), 5); // Capacity preserved
        assert_eq!(cache.get_hit_count(), 0); // Stats reset
        assert_eq!(cache.get_miss_count(), 0);
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

    #[test]
    fn test_invalidate_specific_key() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        cache.insert(0, 10);
        cache.insert(1, 20);
        cache.insert(2, 30);

        // Invalidate key 1
        assert_eq!(cache.invalidate(1), Some(20));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&0), Some(&10));
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&30));

        // Invalidate non-existent key
        assert_eq!(cache.invalidate(5), None);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_invalidate_affects_eviction_order() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(3);
        cache.insert(0, 10);
        cache.insert(1, 20);
        cache.insert(2, 30);

        // Invalidate key 0 (oldest)
        cache.invalidate(0);

        // Now we have 2 entries, capacity is 3
        // Insert key 3 - no eviction yet
        cache.insert(3, 40);
        assert_eq!(cache.get(&0), None);
        assert_eq!(cache.get(&1), Some(&20)); // Still present
        assert_eq!(cache.get(&2), Some(&30));
        assert_eq!(cache.get(&3), Some(&40));

        // Insert key 4 - now we evict key 1 (oldest remaining)
        cache.insert(4, 50);
        assert_eq!(cache.get(&1), None); // Evicted
        assert_eq!(cache.get(&2), Some(&30));
        assert_eq!(cache.get(&3), Some(&40));
        assert_eq!(cache.get(&4), Some(&50));
    }

    #[test]
    fn test_hit_miss_tracking() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        cache.insert(0, 10);
        cache.insert(1, 20);

        // Initial stats
        assert_eq!(cache.get_hit_count(), 0);
        assert_eq!(cache.get_miss_count(), 0);

        // Generate hits and misses
        cache.get(&0); // hit
        cache.get(&1); // hit
        cache.get(&2); // miss
        cache.get(&3); // miss
        cache.get(&0); // hit

        assert_eq!(cache.get_hit_count(), 3);
        assert_eq!(cache.get_miss_count(), 2);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        cache.insert(0, 10);
        cache.insert(1, 20);

        // No accesses yet
        assert_eq!(cache.get_hit_rate(), 0.0);

        // 50% hit rate
        cache.get(&0); // hit
        cache.get(&5); // miss
        assert!((cache.get_hit_rate() - 0.5).abs() < 0.001);

        // 66.7% hit rate (2/3)
        cache.get(&1); // hit
        assert!((cache.get_hit_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_reset_stats() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(5);
        cache.insert(0, 10);
        cache.insert(1, 20);

        // Generate stats
        cache.get(&0); // hit
        cache.get(&5); // miss
        assert_eq!(cache.get_hit_count(), 1);
        assert_eq!(cache.get_miss_count(), 1);

        // Reset stats
        cache.reset_stats();
        assert_eq!(cache.get_hit_count(), 0);
        assert_eq!(cache.get_miss_count(), 0);
        assert_eq!(cache.get_hit_rate(), 0.0);

        // Cache data is preserved
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&0), Some(&10));
        assert_eq!(cache.get(&1), Some(&20));
    }

    #[test]
    fn test_stats_persist_across_operations() {
        let mut cache: RingBufferCache<u32> = RingBufferCache::new(2);
        cache.insert(0, 10);
        cache.get(&0); // hit

        // Insert triggers eviction, but stats persist
        cache.insert(1, 20);
        cache.insert(2, 30); // evicts 0

        assert_eq!(cache.get_hit_count(), 1);
        assert_eq!(cache.get_miss_count(), 0);

        // Try to get evicted key
        cache.get(&0); // miss
        assert_eq!(cache.get_hit_count(), 1);
        assert_eq!(cache.get_miss_count(), 1);
    }
}
