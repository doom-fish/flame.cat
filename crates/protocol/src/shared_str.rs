use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A reference-counted, immutable string for zero-cost cloning.
///
/// Wraps `Arc<str>` so that `.clone()` is a pointer copy + refcount
/// increment instead of a heap allocation. This matters in hot paths
/// like render loops where the same span names are cloned per frame.
///
/// Implements `PartialEq<&str>` so assertions like
/// `assert_eq!(span.name, "main")` work naturally.
#[derive(Debug, Clone, Eq)]
pub struct SharedStr(Arc<str>);

impl SharedStr {
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// --- Equality ---

impl PartialEq for SharedStr {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Fast path: same Arc pointer means equal.
        Arc::ptr_eq(&self.0, &other.0) || *self.0 == *other.0
    }
}

impl PartialEq<str> for SharedStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other
    }
}

impl PartialEq<&str> for SharedStr {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == *other
    }
}

// --- Ordering ---

impl Ord for SharedStr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for SharedStr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// --- Hashing ---

impl std::hash::Hash for SharedStr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (*self.0).hash(state);
    }
}

// --- Deref / Borrow / AsRef ---

impl std::ops::Deref for SharedStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SharedStr {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for SharedStr {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

// --- Conversions ---

impl From<&str> for SharedStr {
    #[inline]
    fn from(s: &str) -> Self {
        SharedStr(Arc::from(s))
    }
}

impl From<String> for SharedStr {
    #[inline]
    fn from(s: String) -> Self {
        SharedStr(Arc::from(s.as_str()))
    }
}

impl From<Arc<str>> for SharedStr {
    #[inline]
    fn from(s: Arc<str>) -> Self {
        SharedStr(s)
    }
}

// --- Display ---

impl std::fmt::Display for SharedStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// --- Serde (hand-rolled to avoid the `rc` feature flag) ---

impl Serialize for SharedStr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SharedStr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(deserializer)?;
        Ok(SharedStr(Arc::from(s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_shares_allocation() {
        let a = SharedStr::from("hello");
        let b = a.clone();
        // Both should deref to the same content.
        assert_eq!(&*a, &*b);
        assert_eq!(a, b);
    }

    #[test]
    fn eq_str() {
        let s = SharedStr::from("test");
        assert_eq!(s, "test");
        assert!(s == "test");
    }

    #[test]
    fn from_string() {
        let s = SharedStr::from(format!("hello {}", 42));
        assert_eq!(s, "hello 42");
    }

    #[test]
    fn deref_and_borrow() {
        let s = SharedStr::from("abc");
        let _: &str = &s;
        let _: &str = s.as_ref();
        let _: &str = std::borrow::Borrow::borrow(&s);
    }

    #[test]
    fn hashmap_lookup_by_str() {
        let mut map = std::collections::HashMap::new();
        map.insert(SharedStr::from("key"), 42);
        assert_eq!(map.get("key"), Some(&42));
    }

    #[test]
    fn serde_roundtrip() {
        let s = SharedStr::from("flame");
        let json = serde_json::to_string(&s).unwrap_or_default();
        assert_eq!(json, "\"flame\"");
        let s2: SharedStr = serde_json::from_str(&json).unwrap_or_else(|_| SharedStr::from(""));
        assert_eq!(s2, "flame");
    }

    #[test]
    fn display() {
        let s = SharedStr::from("hello");
        assert_eq!(format!("{s}"), "hello");
    }

    #[test]
    fn ordering() {
        let a = SharedStr::from("alpha");
        let b = SharedStr::from("beta");
        assert!(a < b);
    }
}
