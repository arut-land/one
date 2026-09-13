//! Renderable chat metadata and immutable transcript rows.
use crate::errors::ChatError;
use std::{
    collections::BTreeMap,
    ops::Bound::{Excluded, Unbounded},
};

#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: u64,
    pub role: ChatRole,
    pub text: String,
    /// Authority acceptance time in Unix milliseconds; zero for older facts.
    pub accepted_at_ms: u64,
    /// Starts a timestamp group after at least five minutes, or at transcript start.
    pub starts_time_group: bool,
    /// Starts a speaker group at a role change or timestamp group boundary.
    pub starts_speaker_group: bool,
}

pub(crate) fn transcript_after(
    messages: &BTreeMap<u64, ChatMessage>,
    after_id: u64,
) -> Vec<ChatMessage> {
    let mut previous = messages
        .range(..=after_id)
        .next_back()
        .map(|(_, message)| message);
    messages
        .range((Excluded(after_id), Unbounded))
        .map(|(_, message)| {
            let mut row = message.clone();
            row.starts_time_group = previous.is_none_or(|previous| {
                message
                    .accepted_at_ms
                    .saturating_sub(previous.accepted_at_ms)
                    >= 300_000
            });
            row.starts_speaker_group = row.starts_time_group
                || previous.is_none_or(|previous| previous.role != message.role);
            previous = Some(message);
            row
        })
        .collect()
}

#[boltffi::data]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChatStatus {
    #[default]
    Idle,
    Sending,
    Failed,
}

#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatState {
    pub id: Option<String>,
    /// Immutable messages through this key are available from `messages_after`.
    pub last_message_id: u64,
    pub status: ChatStatus,
    /// Set exactly when `status` is `Failed`; a surface reads the variant.
    pub error: Option<ChatError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_uses_time_boundaries_roles_and_actual_cursor_predecessor() {
        let rows = [
            (2, ChatRole::User, 0),
            (4, ChatRole::User, 0),
            (6, ChatRole::Assistant, 1),
            (8, ChatRole::Assistant, 300_000),
            (10, ChatRole::Assistant, 600_000),
            (12, ChatRole::Assistant, 599_999),
            (14, ChatRole::User, 599_999),
        ];
        let mut messages: BTreeMap<_, _> = rows
            .into_iter()
            .map(|(id, role, accepted_at_ms)| {
                (
                    id,
                    ChatMessage {
                        id,
                        role,
                        accepted_at_ms,
                        text: id.to_string(),
                        starts_time_group: false,
                        starts_speaker_group: false,
                    },
                )
            })
            .collect();
        let full = transcript_after(&messages, 0);
        assert_eq!(
            full.iter()
                .map(|row| (row.starts_time_group, row.starts_speaker_group))
                .collect::<Vec<_>>(),
            [
                (true, true),
                (false, false),
                (false, true),
                (false, false),
                (true, true),
                (false, false),
                (false, true)
            ]
        );
        for cursor in 0..=15 {
            assert_eq!(
                transcript_after(&messages, cursor),
                full.iter()
                    .filter(|row| row.id > cursor)
                    .cloned()
                    .collect::<Vec<_>>()
            );
        }
        messages.insert(
            16,
            ChatMessage {
                id: 16,
                role: ChatRole::User,
                accepted_at_ms: u64::MAX,
                text: "later".into(),
                starts_time_group: false,
                starts_speaker_group: false,
            },
        );
        assert_eq!(
            &transcript_after(&messages, 0)[..full.len()],
            full.as_slice()
        );
        assert!(transcript_after(&messages, 14)[0].starts_time_group);
        assert!(transcript_after(&messages, u64::MAX).is_empty());
    }
}
