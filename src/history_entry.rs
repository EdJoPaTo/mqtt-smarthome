use std::time::{Duration, SystemTime};

use crate::payload;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    time: SystemTime,
    payload: Box<str>,
}

impl HistoryEntry {
    #[must_use]
    pub fn new<I>(payload: I) -> Self
    where
        I: Into<Box<str>>,
    {
        Self {
            time: SystemTime::now(),
            payload: payload.into(),
        }
    }

    #[must_use]
    pub fn ago(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.time)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn as_boolean(&self) -> bool {
        payload::is_true(&self.payload)
    }

    #[must_use]
    pub fn as_float(&self) -> Option<f32> {
        self.payload.parse::<f32>().ok()
    }

    #[must_use]
    pub const fn payload(&self) -> &str {
        &self.payload
    }
}

#[test]
fn payload_stays_payload() {
    assert_eq!(HistoryEntry::new("42".to_owned()).payload(), "42");
    assert_eq!(HistoryEntry::new("666".to_owned()).payload(), "666");
}

#[test]
fn ago_works() {
    let entry = HistoryEntry::new("42".to_owned());
    std::thread::sleep(Duration::from_millis(25));
    let ago = entry.ago().as_millis();
    println!("ago: {} ms", ago);
    assert!(ago > 20);
    assert!(ago < 300);
}

#[test]
fn payload_as_boolean() {
    assert!(HistoryEntry::new("true".to_owned()).as_boolean());
    assert!(!HistoryEntry::new("false".to_owned()).as_boolean());
}

#[test]
fn payload_as_float() {
    float_eq::assert_float_eq!(
        HistoryEntry::new("42".to_owned()).as_float().unwrap(),
        42.0,
        abs <= 0.1
    );
    float_eq::assert_float_eq!(
        HistoryEntry::new("666".to_owned()).as_float().unwrap(),
        666.0,
        abs <= 0.1
    );
}
