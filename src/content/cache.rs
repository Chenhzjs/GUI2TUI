use std::collections::HashMap;

use crate::semantic::RuntimeNodeId;

use super::ContentBlockId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentCacheBudget {
    pub max_bytes: usize,
    pub max_ranges: usize,
}

impl Default for ContentCacheBudget {
    fn default() -> Self {
        Self {
            max_bytes: 512 * 1024,
            max_ranges: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContentRangeKey {
    pub source: RuntimeNodeId,
    pub start: i32,
    pub end: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedContentRange {
    pub block_id: ContentBlockId,
    pub key: ContentRangeKey,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentCacheMetrics {
    pub ranges: usize,
    pub bytes: usize,
    pub evictions: u64,
}

#[derive(Clone, Debug)]
struct CacheEntry {
    range: LoadedContentRange,
    touched: u64,
}

#[derive(Clone, Debug)]
pub struct ContentCache {
    budget: ContentCacheBudget,
    entries: HashMap<ContentRangeKey, CacheEntry>,
    bytes: usize,
    clock: u64,
    evictions: u64,
}

impl ContentCache {
    pub fn new(budget: ContentCacheBudget) -> Self {
        Self {
            budget,
            entries: HashMap::new(),
            bytes: 0,
            clock: 0,
            evictions: 0,
        }
    }

    pub fn budget(&self) -> ContentCacheBudget {
        self.budget
    }

    pub fn get(&mut self, key: ContentRangeKey) -> Option<&LoadedContentRange> {
        self.clock = self.clock.saturating_add(1);
        let entry = self.entries.get_mut(&key)?;
        entry.touched = self.clock;
        Some(&entry.range)
    }

    pub fn insert(&mut self, range: LoadedContentRange) -> bool {
        let size = range.text.len();
        let key = range.key;
        if self.budget.max_ranges == 0 || size > self.budget.max_bytes {
            return false;
        }
        self.clock = self.clock.saturating_add(1);
        if let Some(old) = self.entries.remove(&range.key) {
            self.bytes = self.bytes.saturating_sub(old.range.text.len());
        }
        self.bytes = self.bytes.saturating_add(size);
        self.entries.insert(
            range.key,
            CacheEntry {
                range,
                touched: self.clock,
            },
        );
        while self.entries.len() > self.budget.max_ranges || self.bytes > self.budget.max_bytes {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(removed.range.text.len());
                self.evictions = self.evictions.saturating_add(1);
            }
        }
        self.entries.contains_key(&key)
    }

    pub fn ranges_for_source(
        &self,
        source: RuntimeNodeId,
    ) -> impl Iterator<Item = &LoadedContentRange> {
        let mut ranges: Vec<_> = self
            .entries
            .values()
            .filter(|entry| entry.range.key.source == source)
            .map(|entry| &entry.range)
            .collect();
        ranges.sort_by_key(|range| range.key.start);
        ranges.into_iter()
    }

    pub fn all_ranges(&self) -> impl Iterator<Item = &LoadedContentRange> {
        self.entries.values().map(|entry| &entry.range)
    }

    pub fn invalidate_source(&mut self, source: RuntimeNodeId) -> usize {
        let keys: Vec<_> = self
            .entries
            .keys()
            .filter(|key| key.source == source)
            .copied()
            .collect();
        for key in &keys {
            if let Some(removed) = self.entries.remove(key) {
                self.bytes = self.bytes.saturating_sub(removed.range.text.len());
            }
        }
        keys.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    pub fn metrics(&self) -> ContentCacheMetrics {
        ContentCacheMetrics {
            ranges: self.entries.len(),
            bytes: self.bytes,
            evictions: self.evictions,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRangeCursor {
    pub source: RuntimeNodeId,
    pub offset: i32,
    pub character_count: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextCursorError {
    InvalidRange,
    NonAdvancingRange,
}

impl TextRangeCursor {
    pub fn advance(&mut self, start: i32, end: i32) -> Result<(), TextCursorError> {
        if start < 0 || end < start || end > self.character_count {
            return Err(TextCursorError::InvalidRange);
        }
        if end <= self.offset {
            return Err(TextCursorError::NonAdvancingRange);
        }
        self.offset = end;
        Ok(())
    }

    pub fn complete(self) -> bool {
        self.offset >= self.character_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(id: u64, start: i32, text: &str) -> LoadedContentRange {
        LoadedContentRange {
            block_id: ContentBlockId::new(id),
            key: ContentRangeKey {
                source: RuntimeNodeId::new(id),
                start,
                end: start + text.chars().count() as i32,
            },
            text: text.to_owned(),
        }
    }

    #[test]
    fn bounded_lru_cache_evicts_old_ranges() {
        let mut cache = ContentCache::new(ContentCacheBudget {
            max_bytes: 8,
            max_ranges: 2,
        });
        assert!(cache.insert(range(1, 0, "four")));
        assert!(cache.insert(range(2, 0, "two")));
        assert!(
            cache
                .get(ContentRangeKey {
                    source: RuntimeNodeId::new(1),
                    start: 0,
                    end: 4,
                })
                .is_some()
        );
        assert!(cache.insert(range(3, 0, "two")));
        assert_eq!(cache.metrics().ranges, 2);
        assert_eq!(cache.metrics().evictions, 1);
        assert!(
            cache
                .ranges_for_source(RuntimeNodeId::new(2))
                .next()
                .is_none()
        );
    }

    #[test]
    fn invalidation_removes_only_changed_source() {
        let mut cache = ContentCache::new(ContentCacheBudget::default());
        cache.insert(range(1, 0, "alpha"));
        cache.insert(range(2, 0, "beta"));
        assert_eq!(cache.invalidate_source(RuntimeNodeId::new(1)), 1);
        assert_eq!(cache.metrics().ranges, 1);
    }

    #[test]
    fn paragraph_cursor_rejects_non_advancing_or_invalid_ranges() {
        let mut cursor = TextRangeCursor {
            source: RuntimeNodeId::new(1),
            offset: 0,
            character_count: 20,
        };
        cursor.advance(0, 7).unwrap();
        assert_eq!(
            cursor.advance(7, 7),
            Err(TextCursorError::NonAdvancingRange)
        );
        assert_eq!(cursor.advance(7, 21), Err(TextCursorError::InvalidRange));
        cursor.advance(7, 20).unwrap();
        assert!(cursor.complete());
    }
}
