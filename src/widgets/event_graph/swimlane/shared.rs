//! Identity-compared handles and the one-slot caches the component keeps
//! between renders.

use std::ops::Deref;
use std::rc::Rc;

/// A shared value compared by identity: two handles are equal when they point
/// at the same allocation, so a prop of this type compares like a pointer
/// however large the value is.
pub(super) struct Shared<T>(pub Rc<T>);

impl<T> Shared<T> {
    pub(super) fn new(value: T) -> Self {
        Shared(Rc::new(value))
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(Rc::clone(&self.0))
    }
}

impl<T> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

/// The last result of a computation and the key it was built from. The key
/// holds the inputs' handles, which keeps their allocations alive: an
/// identity match can never be a recycled address.
pub(super) struct Slot<K, V>(Option<(K, V)>);

impl<K, V> Default for Slot<K, V> {
    fn default() -> Self {
        Slot(None)
    }
}

impl<K: PartialEq, V: Clone> Slot<K, V> {
    /// The remembered value when `key` is unchanged, else `build()`.
    pub(super) fn get(&mut self, key: K, build: impl FnOnce() -> V) -> V {
        if let Some((last, value)) = &self.0 {
            if *last == key {
                return value.clone();
            }
        }
        let value = build();
        self.0 = Some((key, value.clone()));
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_builds_once_per_key() {
        let mut slot: Slot<u32, Rc<String>> = Slot::default();
        let mut builds = 0;
        let first = slot.get(1, || {
            builds += 1;
            Rc::new("one".to_string())
        });
        let again = slot.get(1, || {
            builds += 1;
            Rc::new("rebuilt".to_string())
        });
        assert_eq!(builds, 1);
        assert!(Rc::ptr_eq(&first, &again));
        let next = slot.get(2, || {
            builds += 1;
            Rc::new("two".to_string())
        });
        assert_eq!((builds, next.as_str()), (2, "two"));
    }

    #[test]
    fn shared_handles_compare_by_identity_not_by_value() {
        let events = Shared::new(vec![1, 2]);
        let handle = events.clone();
        let twin = Shared::new(vec![1, 2]);
        assert!(events == handle);
        assert!(events != twin);
    }
}
