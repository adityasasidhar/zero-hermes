//! Conversation context: a `Vec<Message>` with optional compaction.
//!
//! Compaction is intentionally simple: when the message count exceeds
//! `context_window`, drop the oldest non-system messages until we are back
//! under the limit. The system message is always preserved (the caller
//! passes it separately).

use crate::agent::tool::Message;

/// A bounded conversation history.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub messages: Vec<Message>,
    pub max_messages: usize,
}

impl Context {
    /// Create a new context with no compaction (caller responsible).
    pub fn unbounded() -> Self {
        Self {
            messages: Vec::new(),
            max_messages: usize::MAX,
        }
    }

    /// Create a context that compacts when it exceeds `max_messages`.
    pub fn bounded(max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages,
        }
    }

    /// Append a message and compact if needed.
    pub fn push(&mut self, msg: Message) {
        self.messages.push(msg);
        self.compact();
    }

    /// Drop oldest messages until we are within `max_messages`.
    pub fn compact(&mut self) {
        if self.max_messages == 0 || self.max_messages == usize::MAX {
            return;
        }
        while self.messages.len() > self.max_messages {
            // never drop the first message if it's the only one — keep at
            // least one entry so the context is never empty.
            if self.messages.len() <= 1 {
                break;
            }
            self.messages.remove(0);
        }
    }

    /// Borrow the underlying message list.
    pub fn as_slice(&self) -> &[Message] {
        &self.messages
    }

    /// Push many messages at once (each is compacted).
    pub fn extend<I: IntoIterator<Item = Message>>(&mut self, iter: I) {
        for m in iter {
            self.push(m);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_drops_oldest() {
        let mut ctx = Context::bounded(3);
        for i in 0..6 {
            ctx.push(Message::user(format!("msg {i}")));
        }
        assert_eq!(ctx.messages.len(), 3);
        assert_eq!(ctx.messages[0].text(), "msg 3");
        assert_eq!(ctx.messages[2].text(), "msg 5");
    }

    #[test]
    fn unbounded_never_drops() {
        let mut ctx = Context::unbounded();
        for i in 0..100 {
            ctx.push(Message::user(format!("m{i}")));
        }
        assert_eq!(ctx.messages.len(), 100);
    }

    #[test]
    fn always_keeps_one() {
        let mut ctx = Context::bounded(0);
        ctx.push(Message::user("only"));
        assert_eq!(ctx.messages.len(), 1);
    }
}
