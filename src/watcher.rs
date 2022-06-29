use tokio::sync::mpsc::{channel, Receiver, Sender};

pub type ChannelPayload = (String, String);

pub struct Watcher {
    allow_retained: bool,
    filter: Box<str>,
    sender: Sender<ChannelPayload>,
}

impl Watcher {
    pub fn new(mqtt_topic_filter: &str, allow_retained: bool) -> (Self, Receiver<ChannelPayload>) {
        assert!(
            rumqttc::mqttbytes::valid_filter(mqtt_topic_filter),
            "topic filter is not valid"
        );
        let (sender, receiver) = channel(25);
        let watcher = Self {
            allow_retained,
            filter: mqtt_topic_filter.into(),
            sender,
        };
        (watcher, receiver)
    }

    #[must_use]
    fn is_match(&self, topic: &str, retained: bool) -> bool {
        if retained && !self.allow_retained {
            return false;
        }
        rumqttc::mqttbytes::matches(topic, &self.filter)
    }

    pub fn matching_sender(&self, topic: &str, retained: bool) -> Option<Sender<ChannelPayload>> {
        if self.is_match(topic, retained) {
            Some(self.sender.clone())
        } else {
            None
        }
    }
}

#[test]
fn is_match_retained_allowed() {
    let (watcher, _receiver) = Watcher::new("#", true);
    assert!(watcher.is_match("foo/bar", true));
    assert!(watcher.is_match("foo/bar", false));
}

#[test]
fn is_match_retained_not_allowed() {
    let (watcher, _receiver) = Watcher::new("#", false);
    assert!(!watcher.is_match("foo/bar", true));
    assert!(watcher.is_match("foo/bar", false));
}

#[test]
fn is_match_matches() {
    let (watcher, _receiver) = Watcher::new("foo/#", false);
    assert!(watcher.is_match("foo/bar", false));
    assert!(!watcher.is_match("whatever/else", false));
}

#[test]
#[should_panic = "topic filter is not valid"]
fn bad_filter_panics() {
    Watcher::new("#/whatever", false);
}
