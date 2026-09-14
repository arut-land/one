/// A non-empty, single-line conversation title bounded for navigation lists.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ConversationTitle(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidConversationTitle;

impl std::fmt::Display for InvalidConversationTitle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a conversation title must contain text")
    }
}

impl std::error::Error for InvalidConversationTitle {}

impl TryFrom<String> for ConversationTitle {
    type Error = InvalidConversationTitle;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl TryFrom<&str> for ConversationTitle {
    type Error = InvalidConversationTitle;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let normalized = value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(48)
            .collect::<String>();
        (!normalized.is_empty())
            .then_some(Self(normalized))
            .ok_or(InvalidConversationTitle)
    }
}

impl AsRef<str> for ConversationTitle {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ConversationTitle> for String {
    fn from(title: ConversationTitle) -> Self {
        title.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_normalizes_bounds_and_rejects_empty_titles() {
        assert_eq!(
            ConversationTitle::try_from("  Project   notes  ".to_owned())
                .unwrap()
                .as_ref(),
            "Project notes"
        );
        assert_eq!(
            ConversationTitle::try_from("x".repeat(80))
                .unwrap()
                .as_ref()
                .chars()
                .count(),
            48
        );
        assert_eq!(
            ConversationTitle::try_from(" \n ".to_owned()),
            Err(InvalidConversationTitle)
        );
    }
}
