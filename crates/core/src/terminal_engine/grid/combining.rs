//! Bounded, per-terminal interning for repeated composed cells.
//!
//! A set-associative table keeps lookup work constant. Entries are immutable:
//! appending another mark produces a new value and cannot change an older cell.

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use super::super::types::{Cell, CellExtra, MAX_COMBINING_BYTES};

const BUCKETS: usize = 64;
const WAYS: usize = 4;
// Avoid pinning large OSC 8 strings after their last visible cell is erased.
const MAX_CACHED_HYPERLINK_BYTES: usize = 1024;

pub(super) struct CombiningCache {
    slots: [[Option<Arc<CellExtra>>; WAYS]; BUCKETS],
    next: [u8; BUCKETS],
}

impl Default for CombiningCache {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| std::array::from_fn(|_| None)),
            next: [0; BUCKETS],
        }
    }
}

impl CombiningCache {
    pub(super) fn clear(&mut self) {
        for bucket in &mut self.slots {
            bucket.fill(None);
        }
        self.next.fill(0);
    }

    pub(super) fn append(&mut self, cell: &mut Cell, character: char) {
        let suffix = cell.combining();
        let mut encoded = [0; 4];
        let encoded = character.encode_utf8(&mut encoded).as_bytes();
        let len = suffix.len() + encoded.len();
        if len > MAX_COMBINING_BYTES {
            return;
        }
        let hyperlink = cell.hyperlink();
        if hyperlink.is_some_and(|link| {
            link.id.capacity().saturating_add(link.uri.capacity()) > MAX_CACHED_HYPERLINK_BYTES
        }) {
            cell.push_combining(character);
            return;
        }

        let link_pointer = hyperlink.map_or(std::ptr::null(), Arc::as_ptr);
        let mut hasher = DefaultHasher::new();
        // Use the full resulting byte string as the key; grouping does not
        // affect identity when a value was built over different feed chunks.
        hasher.write(suffix.as_bytes());
        hasher.write(encoded);
        link_pointer.hash(&mut hasher);
        let bucket = hasher.finish() as usize % BUCKETS;
        for entry in self.slots[bucket].iter().flatten() {
            let same_link = entry
                .hyperlink
                .as_ref()
                .map_or(std::ptr::null(), Arc::as_ptr)
                == link_pointer;
            if same_link
                && entry.combining.len() == len
                && entry.combining.as_bytes().starts_with(suffix.as_bytes())
                && entry.combining.as_bytes()[suffix.len()..] == *encoded
            {
                cell.extra = Some(Arc::clone(entry));
                return;
            }
        }

        let mut combining = String::with_capacity(len);
        combining.push_str(suffix);
        combining.push(character);
        let extra = Arc::new(CellExtra {
            combining,
            hyperlink: hyperlink.cloned(),
        });
        let slot = usize::from(self.next[bucket]);
        self.next[bucket] = ((slot + 1) % WAYS) as u8;
        self.slots[bucket][slot] = Some(Arc::clone(&extra));
        cell.extra = Some(extra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_engine::Hyperlink;

    #[test]
    fn repeated_compositions_share_immutable_metadata() {
        let mut cache = CombiningCache::default();
        let mut first = Cell::default();
        let mut second = Cell::default();
        cache.append(&mut first, '\u{301}');
        cache.append(&mut second, '\u{301}');
        assert!(Arc::ptr_eq(
            first.extra.as_ref().unwrap(),
            second.extra.as_ref().unwrap()
        ));
        cache.append(&mut second, '\u{308}');
        assert_eq!(first.combining(), "\u{301}");
        assert_eq!(second.combining(), "\u{301}\u{308}");
        assert!(!Arc::ptr_eq(
            first.extra.as_ref().unwrap(),
            second.extra.as_ref().unwrap()
        ));
    }

    #[test]
    fn distinct_hyperlinks_never_share_composed_metadata() {
        let mut cache = CombiningCache::default();
        let mut first = Cell {
            extra: Some(Arc::new(CellExtra {
                combining: String::new(),
                hyperlink: Some(Arc::new(Hyperlink {
                    id: "first".into(),
                    uri: "https://one.test".into(),
                })),
            })),
            ..Cell::default()
        };
        let mut second = Cell {
            extra: Some(Arc::new(CellExtra {
                combining: String::new(),
                hyperlink: Some(Arc::new(Hyperlink {
                    id: "second".into(),
                    uri: "https://two.test".into(),
                })),
            })),
            ..Cell::default()
        };
        cache.append(&mut first, '\u{301}');
        cache.append(&mut second, '\u{301}');
        assert_eq!(first.hyperlink().unwrap().id, "first");
        assert_eq!(second.hyperlink().unwrap().id, "second");
        assert!(!Arc::ptr_eq(
            first.extra.as_ref().unwrap(),
            second.extra.as_ref().unwrap()
        ));
    }

    #[test]
    fn cache_storage_and_suffixes_are_bounded_under_unique_input() {
        let mut cache = CombiningCache::default();
        for index in 0..10_000 {
            let mut cell = Cell::default();
            cache.append(&mut cell, char::from_u32(0x1000 + index).unwrap());
        }
        let slots: Vec<_> = cache.slots.iter().flatten().flatten().collect();
        assert!(slots.len() <= BUCKETS * WAYS);
        assert!(
            slots
                .iter()
                .all(|entry| entry.combining.capacity() <= MAX_COMBINING_BYTES)
        );
        let mut cell = Cell::default();
        for _ in 0..1000 {
            cache.append(&mut cell, '\u{1d185}');
        }
        assert!(cell.combining().len() <= MAX_COMBINING_BYTES);
        cache.clear();
        assert!(cache.slots.iter().flatten().all(Option::is_none));
    }

    #[test]
    fn large_hyperlinks_are_not_retained_by_the_cache() {
        let link = Arc::new(Hyperlink {
            id: String::new(),
            uri: "x".repeat(MAX_CACHED_HYPERLINK_BYTES + 1),
        });
        let mut cell = Cell {
            extra: Some(Arc::new(CellExtra {
                combining: String::new(),
                hyperlink: Some(Arc::clone(&link)),
            })),
            ..Cell::default()
        };
        let mut cache = CombiningCache::default();
        cache.append(&mut cell, '\u{301}');
        assert_eq!(cell.combining(), "\u{301}");
        assert!(cache.slots.iter().flatten().all(Option::is_none));
        drop(cell);
        assert_eq!(Arc::strong_count(&link), 1);
    }
}
