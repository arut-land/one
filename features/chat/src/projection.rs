//! Renderable chat metadata and immutable transcript rows.
use crate::errors::ChatError;
use std::collections::BTreeMap;

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
    /// Start time of the preceding timestamp group. A surface compares its
    /// local calendar day with this instant before repeating a date label.
    pub previous_time_group_at_ms: Option<u64>,
    /// Starts a speaker group at a role change or timestamp group boundary.
    pub starts_speaker_group: bool,
    /// Ends a speaker group: the next row starts one, or this is the last row
    /// the transcript has. A surface spaces after a row from this rather than
    /// reading the row behind it.
    ///
    /// The last row's value is what the transcript says today; a later batch
    /// that keeps the same speaker makes it false for whoever reads that row
    /// next. Every accepted batch so far changes speaker, so a cached row does
    /// not go stale in practice.
    pub ends_speaker_group: bool,
}

pub(crate) fn transcript_after(
    messages: &BTreeMap<u64, ChatMessage>,
    after_id: u64,
) -> Vec<ChatMessage> {
    let mut previous: Option<&ChatMessage> = None;
    let mut previous_time_group_at_ms: Option<u64> = None;
    let mut rows: Vec<ChatMessage> = messages
        .iter()
        .filter_map(|(id, message)| {
            let mut row = message.clone();
            row.starts_time_group = previous.is_none_or(|previous| {
                message
                    .accepted_at_ms
                    .saturating_sub(previous.accepted_at_ms)
                    >= 300_000
            });
            row.previous_time_group_at_ms = if row.starts_time_group {
                previous_time_group_at_ms
            } else {
                None
            };
            row.starts_speaker_group = row.starts_time_group
                || previous.is_none_or(|previous| previous.role != message.role);
            if row.starts_time_group {
                previous_time_group_at_ms = Some(message.accepted_at_ms);
            }
            previous = Some(message);
            (*id > after_id).then_some(row)
        })
        .collect();
    for index in (0..rows.len()).rev() {
        let ends = rows
            .get(index + 1)
            .is_none_or(|next| next.starts_speaker_group);
        rows[index].ends_speaker_group = ends;
    }
    rows
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
    /// A send would be attempted: nothing is in flight and the conversation
    /// scope is live. A surface also requires composer text, which it reads
    /// from `ComposerState`.
    ///
    /// Derived from the fields above rather than set on its own. It is a field
    /// and not a method because FFI data types carry no behavior.
    pub can_send: bool,
    /// `status` is `Sending`. Derived; see [`ChatState::can_send`].
    pub is_sending: bool,
    /// No message has been accepted yet. Derived; see [`ChatState::can_send`].
    pub is_empty: bool,
}

impl ChatState {
    /// Recomputes the derived fields. Every write to this projection ends here,
    /// so the three are never stale with respect to what they are derived from.
    pub(crate) fn derive(&mut self, cancelled: bool) {
        self.is_sending = self.status == ChatStatus::Sending;
        self.is_empty = self.last_message_id == 0;
        self.can_send = !self.is_sending && !cancelled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u64, role: ChatRole, accepted_at_ms: u64) -> ChatMessage {
        ChatMessage {
            id,
            role,
            accepted_at_ms,
            text: id.to_string(),
            starts_time_group: false,
            previous_time_group_at_ms: None,
            starts_speaker_group: false,
            ends_speaker_group: false,
        }
    }

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
            .map(|(id, role, accepted_at_ms)| (id, row(id, role, accepted_at_ms)))
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
        assert_eq!(full[0].previous_time_group_at_ms, None);
        assert_eq!(full[4].previous_time_group_at_ms, Some(0));
        for cursor in 0..=15 {
            assert_eq!(
                transcript_after(&messages, cursor),
                full.iter()
                    .filter(|row| row.id > cursor)
                    .cloned()
                    .collect::<Vec<_>>()
            );
        }
        messages.insert(16, row(16, ChatRole::User, u64::MAX));
        assert_eq!(
            &transcript_after(&messages, 0)[..full.len()],
            full.as_slice()
        );
        assert!(transcript_after(&messages, 14)[0].starts_time_group);
        assert!(transcript_after(&messages, u64::MAX).is_empty());
    }

    #[test]
    fn a_row_ends_a_speaker_group_when_the_next_row_starts_one() {
        let messages: BTreeMap<_, _> = [
            (2, ChatRole::User, 0),
            (4, ChatRole::User, 0),
            (6, ChatRole::Assistant, 1),
        ]
        .into_iter()
        .map(|(id, role, accepted_at_ms)| (id, row(id, role, accepted_at_ms)))
        .collect();
        assert_eq!(
            transcript_after(&messages, 0)
                .iter()
                .map(|row| row.ends_speaker_group)
                .collect::<Vec<_>>(),
            [false, true, true],
            "the last row of the transcript ends its group"
        );
    }
}
