use std::collections::HashSet;

pub struct Subscriptions(pub HashSet<String>);

impl Subscriptions {
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    /// Returns `true` when the given filter was added.
    pub fn add(&mut self, filter: &str) -> bool {
        let already_subscribed = self
            .0
            .iter()
            .any(|existing| rumqttc::matches(filter, existing));
        if already_subscribed {
            return false;
        }
        self.0
            .retain(|existing| !rumqttc::matches(existing, filter));
        self.0.insert(filter.to_owned())
    }

    #[cfg(test)]
    fn as_sorted(&self) -> Vec<&str> {
        let mut vec = self.0.iter().map(|filter| &**filter).collect::<Vec<_>>();
        vec.sort_unstable();
        vec
    }
}

#[test]
fn multiple_same_adds() {
    let mut subs = Subscriptions::new();
    assert_eq!(subs.0.len(), 0);
    assert!(subs.add("foo/bar"));
    assert_eq!(subs.as_sorted(), ["foo/bar"]);
    assert!(!subs.add("foo/bar"));
    assert_eq!(subs.as_sorted(), ["foo/bar"]);
}

#[test]
fn multiple_unique_adds() {
    let mut subs = Subscriptions::new();
    assert_eq!(subs.0.len(), 0);
    assert!(subs.add("foo/bar"));
    assert_eq!(subs.as_sorted(), ["foo/bar"]);
    assert!(subs.add("something"));
    assert_eq!(subs.as_sorted(), ["foo/bar", "something"]);
}

#[test]
fn ignore_already_included() {
    let mut subs = Subscriptions::new();
    assert_eq!(subs.0.len(), 0);
    assert!(subs.add("foo/+"));
    assert_eq!(subs.as_sorted(), ["foo/+"]);
    assert!(!subs.add("foo/bar"));
    assert_eq!(subs.as_sorted(), ["foo/+"]);
}

#[test]
fn replace_existing_when_broader_filter_added() {
    let mut subs = Subscriptions::new();
    assert_eq!(subs.0.len(), 0);
    assert!(subs.add("foo/bar"));
    assert_eq!(subs.as_sorted(), ["foo/bar"]);
    assert!(subs.add("foo/something"));
    assert_eq!(subs.as_sorted(), ["foo/bar", "foo/something"]);
    assert!(subs.add("foo/+"));
    assert_eq!(subs.as_sorted(), ["foo/+"]);
}
