pub(crate) fn respond(message: &str) -> String {
    format!("You said: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_mock_response() {
        assert_eq!(respond("hello"), "You said: hello");
    }
}
