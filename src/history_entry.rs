use core::time::Duration;
use std::time::SystemTime;

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
        self.payload
            .split(char::is_whitespace)
            .find(|o| !o.is_empty())?
            .parse::<f32>()
            .ok()
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
    println!("ago: {ago} ms");
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
    fn t(input: &str, expected: Option<f32>) {
        let actual = HistoryEntry::new(input.to_owned()).as_float();
        match (actual, expected) {
            (None, None) => {} // All fine
            (Some(actual), Some(expected)) => {
                float_eq::assert_float_eq!(actual, expected, abs <= 0.1);
            }
            _ => panic!("Assertion failed:\n{actual:?} should be\n{expected:?}"),
        }
    }

    t("", None);
    t("test", None);
    t("42", Some(42.0));
    t("666", Some(666.0));
    t("12.3 °C", Some(12.3));
    t(" 2.4 °C", Some(2.4));
}
