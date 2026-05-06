//! In-memory cache for SACS `LIST_REMOTE` responses.
//!
//! Lives inside the agent so multiple `__complete` invocations
//! across separate CLI processes share results. TTL is 60 s by
//! design (`cli-autosuggest.md` §9). Failures are NOT cached — a
//! transient 503 from a provider must not poison subsequent
//! completions.
//!
//! Invalidation walks **prefix → root** ignoring `kind_filter`.
//! Concretely, a successful upload to `backup/snap/file.md`
//! drops every cached entry whose prefix is `Some("backup/snap")`,
//! `Some("backup")`, or `None`, regardless of File / Directory /
//! Both filtering. Wildcard purge over-invalidates by design — a
//! refilled cache costs one PROPFIND per prefix and freshness is
//! more important than HTTP economy here.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zz_drop_core::agent_proto::{EntryKindFilter, RemoteListEntry};

/// 60-second TTL per design §9. Values older than this are dropped
/// at lookup time; that lazy approach keeps the agent's hot path
/// allocation-free.
pub const DEFAULT_TTL: Duration = Duration::from_secs(60);

/// Wall-clock dependency. Tests inject a deterministic
/// implementation; production uses [`SystemClock`].
pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

#[derive(Default, Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CacheKey {
    prefix: Option<String>,
    kind_filter: KindKey,
}

/// `EntryKindFilter` doesn't `Ord` itself — it's only `Hash + Eq`
/// in core. We keep the wire enum unchanged and project it onto a
/// small ordered helper here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum KindKey {
    File,
    Directory,
    Both,
}

impl From<EntryKindFilter> for KindKey {
    fn from(k: EntryKindFilter) -> Self {
        match k {
            EntryKindFilter::File => KindKey::File,
            EntryKindFilter::Directory => KindKey::Directory,
            EntryKindFilter::Both => KindKey::Both,
        }
    }
}

#[derive(Clone, Debug)]
struct CacheEntry {
    fetched_at: SystemTime,
    entries: Vec<RemoteListEntry>,
    truncated: bool,
}

/// Hit returned to the dispatcher when the cache served the
/// request. Convertible into the wire response.
#[derive(Clone, Debug)]
pub struct Hit {
    pub fetched_at: SystemTime,
    pub entries: Vec<RemoteListEntry>,
    pub truncated: bool,
}

pub struct ListCache {
    map: BTreeMap<CacheKey, CacheEntry>,
    ttl: Duration,
    clock: Box<dyn Clock>,
}

impl Default for ListCache {
    fn default() -> Self {
        Self::with_clock(Box::new(SystemClock))
    }
}

impl std::fmt::Debug for ListCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Names of remote entries flow through here; redact in
        // case a stray `dbg!(state)` happens elsewhere.
        f.debug_struct("ListCache")
            .field("entries_count", &self.map.len())
            .field("ttl_secs", &self.ttl.as_secs())
            .finish()
    }
}

impl ListCache {
    pub fn with_clock(clock: Box<dyn Clock>) -> Self {
        Self {
            map: BTreeMap::new(),
            ttl: DEFAULT_TTL,
            clock,
        }
    }

    #[cfg(test)]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Look up a cached response. Returns `None` when:
    /// - the key was never populated;
    /// - the entry is older than `ttl` (we drop it lazily here).
    pub fn get(&mut self, prefix: Option<&str>, kind_filter: EntryKindFilter) -> Option<Hit> {
        let key = self.make_key(prefix, kind_filter);
        let now = self.clock.now();
        match self.map.get(&key) {
            Some(entry) => {
                let age = now.duration_since(entry.fetched_at).unwrap_or(Duration::ZERO);
                if age > self.ttl {
                    self.map.remove(&key);
                    None
                } else {
                    Some(Hit {
                        fetched_at: entry.fetched_at,
                        entries: entry.entries.clone(),
                        truncated: entry.truncated,
                    })
                }
            }
            None => None,
        }
    }

    /// Store a successful list response. Failures must never
    /// reach this method: they get returned to the caller and
    /// dropped on the floor.
    pub fn put(
        &mut self,
        prefix: Option<&str>,
        kind_filter: EntryKindFilter,
        entries: Vec<RemoteListEntry>,
        truncated: bool,
    ) {
        let key = self.make_key(prefix, kind_filter);
        self.map.insert(
            key,
            CacheEntry {
                fetched_at: self.clock.now(),
                entries,
                truncated,
            },
        );
    }

    /// Drop every entry whose prefix is `target` or any of its
    /// parents (up to the root). All `kind_filter` variants for
    /// each level are purged together — see the module docstring
    /// for the rationale.
    pub fn invalidate_prefix_to_root(&mut self, target: Option<&str>) {
        for prefix in walk_to_root(target) {
            self.map.retain(|key, _| key.prefix != prefix);
        }
    }

    /// Wipe everything. Called on `Lock`, `UpdateProfile`,
    /// `UpdateProfileSet`, `SetActiveAlias`, and TTL-driven auto-lock —
    /// any state change that might affect what "the active provider"
    /// would return.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// For test introspection only.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    fn make_key(&self, prefix: Option<&str>, kind_filter: EntryKindFilter) -> CacheKey {
        CacheKey {
            prefix: prefix.map(String::from),
            kind_filter: kind_filter.into(),
        }
    }
}

/// Translate a `cached_at` `SystemTime` into a unix epoch seconds
/// `u64` for the wire response.
pub fn epoch_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Yield the full chain of prefixes from `target` up to (and
/// including) the root: `Some("a/b/c")` → `[Some("a/b/c"),
/// Some("a/b"), Some("a"), None]`. Empty / pure-slash / `..`
/// segments are dropped as a defence against malformed input.
pub fn walk_to_root(target: Option<&str>) -> Vec<Option<String>> {
    let mut chain: Vec<Option<String>> = Vec::new();
    if let Some(p) = target {
        let normalised = p.trim_matches('/');
        let segments: Vec<&str> = normalised
            .split('/')
            .filter(|s| !s.is_empty() && *s != "." && *s != "..")
            .collect();
        for i in (1..=segments.len()).rev() {
            chain.push(Some(segments[..i].join("/")));
        }
    }
    chain.push(None);
    chain
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FrozenClock(Mutex<SystemTime>);

    impl FrozenClock {
        fn new(t: SystemTime) -> Self {
            Self(Mutex::new(t))
        }
        fn advance(&self, d: Duration) {
            let mut g = self.0.lock().unwrap();
            *g += d;
        }
    }

    impl Clock for FrozenClock {
        fn now(&self) -> SystemTime {
            *self.0.lock().unwrap()
        }
    }

    fn epoch(s: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(s)
    }

    fn entry(name: &str) -> RemoteListEntry {
        RemoteListEntry {
            name: name.into(),
            size: Some(100),
            kind: zz_drop_core::agent_proto::RemoteKind::File,
            mtime_secs: None,
        }
    }

    #[test]
    fn cold_cache_returns_none() {
        let mut c = ListCache::default();
        assert!(c.get(None, EntryKindFilter::Both).is_none());
    }

    #[test]
    fn put_then_get_round_trips() {
        let mut c = ListCache::default();
        c.put(None, EntryKindFilter::Both, vec![entry("a")], false);
        let hit = c.get(None, EntryKindFilter::Both).unwrap();
        assert_eq!(hit.entries.len(), 1);
        assert!(!hit.truncated);
    }

    #[test]
    fn ttl_drops_stale_entries_at_lookup_time() {
        let clock = std::sync::Arc::new(FrozenClock::new(epoch(1_000_000)));
        let clock_box: Box<dyn Clock> = Box::new(ClockShim(clock.clone()));
        let mut c = ListCache::with_clock(clock_box).with_ttl(Duration::from_secs(60));
        c.put(None, EntryKindFilter::Both, vec![entry("a")], false);
        assert!(c.get(None, EntryKindFilter::Both).is_some());
        clock.advance(Duration::from_secs(61));
        assert!(c.get(None, EntryKindFilter::Both).is_none());
        // A hit that expired must NOT linger in the map.
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn keys_distinguish_kind_filter() {
        let mut c = ListCache::default();
        c.put(None, EntryKindFilter::File, vec![entry("file")], false);
        c.put(None, EntryKindFilter::Directory, vec![entry("dir")], false);
        let f = c.get(None, EntryKindFilter::File).unwrap();
        let d = c.get(None, EntryKindFilter::Directory).unwrap();
        assert_eq!(f.entries[0].name, "file");
        assert_eq!(d.entries[0].name, "dir");
    }

    #[test]
    fn invalidate_walks_prefix_to_root() {
        let mut c = ListCache::default();
        c.put(Some("backup/snap"), EntryKindFilter::Both, vec![entry("a")], false);
        c.put(Some("backup"), EntryKindFilter::File, vec![entry("b")], false);
        c.put(None, EntryKindFilter::Directory, vec![entry("c")], false);
        c.put(Some("docs"), EntryKindFilter::Both, vec![entry("d")], false);
        assert_eq!(c.len(), 4);

        c.invalidate_prefix_to_root(Some("backup/snap"));

        // backup/snap, backup, root → all 3 dropped, regardless of
        // kind_filter. `docs` is unrelated, kept.
        assert!(c.get(Some("backup/snap"), EntryKindFilter::Both).is_none());
        assert!(c.get(Some("backup"), EntryKindFilter::File).is_none());
        assert!(c.get(None, EntryKindFilter::Directory).is_none());
        assert!(c.get(Some("docs"), EntryKindFilter::Both).is_some());
    }

    #[test]
    fn invalidate_with_root_target_drops_only_root_keys() {
        let mut c = ListCache::default();
        c.put(None, EntryKindFilter::Both, vec![entry("root")], false);
        c.put(Some("docs"), EntryKindFilter::Both, vec![entry("docs")], false);
        c.invalidate_prefix_to_root(None);
        assert!(c.get(None, EntryKindFilter::Both).is_none());
        assert!(c.get(Some("docs"), EntryKindFilter::Both).is_some());
    }

    #[test]
    fn clear_drops_everything() {
        let mut c = ListCache::default();
        c.put(None, EntryKindFilter::Both, vec![entry("root")], false);
        c.put(Some("docs"), EntryKindFilter::Both, vec![entry("docs")], false);
        c.clear();
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn walk_to_root_unwinds_segments() {
        assert_eq!(
            walk_to_root(Some("a/b/c")),
            vec![
                Some("a/b/c".into()),
                Some("a/b".into()),
                Some("a".into()),
                None,
            ]
        );
        assert_eq!(walk_to_root(Some("docs/")), vec![Some("docs".into()), None]);
        assert_eq!(walk_to_root(None), vec![None]);
        // Defence against malformed input — `..` and empty segments
        // are dropped before walking.
        assert_eq!(
            walk_to_root(Some("a/../b//c")),
            vec![Some("a/b/c".into()), Some("a/b".into()), Some("a".into()), None]
        );
    }

    #[test]
    fn epoch_secs_round_trip() {
        let t = epoch(1_700_000_000);
        assert_eq!(epoch_secs(t), 1_700_000_000);
    }

    /// Bridge struct so the test can advance the same FrozenClock
    /// instance the cache holds via `Box<dyn Clock>`.
    struct ClockShim(std::sync::Arc<FrozenClock>);
    impl Clock for ClockShim {
        fn now(&self) -> SystemTime {
            self.0.now()
        }
    }
}
