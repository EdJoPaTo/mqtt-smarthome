#[must_use]
pub struct Watcher<Action> {
    allow_retained: bool,
    filter: Box<str>,
    action: Action,
}

impl<Action> Watcher<Action> {
    #[track_caller]
    pub fn new(filter: &str, allow_retained: bool, action: Action) -> Self {
        assert!(rumqttc::valid_filter(filter), "topic filter is not valid");
        Self {
            allow_retained,
            filter: filter.into(),
            action,
        }
    }

    #[must_use]
    pub fn matching(&self, topic: &str, retained: bool) -> Option<&Action> {
        if retained && !self.allow_retained {
            return None;
        }
        rumqttc::matches(topic, &self.filter).then_some(&self.action)
    }
}

#[test]
fn retained_allowed() {
    let watcher = Watcher::new("foo/#", true, ());
    assert!(watcher.matching("foo/bar", true).is_some());
    assert!(watcher.matching("foo/bar", false).is_some());
}

#[test]
fn retained_not_allowed() {
    let watcher = Watcher::new("foo/#", false, ());
    assert!(watcher.matching("foo/bar", true).is_none());
    assert!(watcher.matching("foo/bar", false).is_some());
}

#[test]
fn subtopicfilter() {
    let watcher = Watcher::new("foo/#", false, ());
    assert!(watcher.matching("foo/bar", false).is_some());
    assert!(watcher.matching("whatever/else", false).is_none());
}

#[test]
#[should_panic = "topic filter is not valid"]
fn bad_filter_panics() {
    _ = Watcher::new("#/whatever", false, ());
}
