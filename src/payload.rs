/// Detect common `true` / `false` states in a string payload.
#[must_use]
pub fn is_true(payload: &str) -> bool {
    match payload {
        "true" | "True" | "TRUE" | "on" | "On" | "ON" | "online" | "Online" | "ONLINE" | "1"
        | "2" => true,
        "false" | "False" | "FALSE" | "off" | "Off" | "OFF" | "offline" | "Offline" | "OFFLINE"
        | "0" => false,
        _ => {
            eprintln!("WARNING is_true unclear, assumes true: {payload:?}");
            true
        }
    }
}

/// Tries to parse a string payload to a `f32`.
///
/// Allows for common patterns like `12.3 °C` which returns `12.3`.
/// Splits at whitespaces and expects the number at the front.
#[must_use]
pub fn as_f32(payload: &str) -> Option<f32> {
    payload
        .split(char::is_whitespace)
        .find(|str| !str.is_empty())?
        .parse::<f32>()
        .ok()
        .filter(|float| float.is_finite())
}

#[cfg(test)]
mod tests {
    #[rstest::rstest]
    fn is_true(#[values("on", "1", "true")] payload: &str) {
        assert!(super::is_true(payload));
    }

    #[rstest::rstest]
    fn is_false(#[values("off", "0", "false")] payload: &str) {
        assert!(!super::is_true(payload));
    }

    #[rstest::rstest]
    #[case::empty("", None)]
    #[case::text("test", None)]
    #[case::number("42", Some(42.0))]
    #[case::number("666", Some(666.0))]
    #[case::unit("12.3 °C", Some(12.3))]
    #[case::indent(" 2.4 °C", Some(2.4))]
    fn as_f32(#[case] input: &str, #[case] expected: Option<f32>) {
        let actual = super::as_f32(input);
        match (actual, expected) {
            (None, None) => {} // All fine
            (Some(actual), Some(expected)) => {
                float_eq::assert_float_eq!(actual, expected, abs <= 0.01);
            }
            _ => panic!("Assertion failed:\n{actual:?} should be\n{expected:?}"),
        }
    }
}
