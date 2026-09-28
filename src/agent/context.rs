//! Conversation compaction.
//!
//! The agent loop keeps history as a plain `Vec<Message>` and hands the
//! whole thing to the provider each iteration, so an unbounded history is
//! an unbounded request — a long-lived gateway chat eventually gets
//! rejected for exceeding the model's context.
//!
//! Compaction drops the oldest messages, but it cannot do so naively. A
//! `tool_result` block is only valid immediately after the assistant
//! message carrying the matching `tool_use`; cutting between the two
//! leaves an orphan and the Messages API rejects the request outright.
//! [`compact_history`] therefore never cuts to an arbitrary index — it
//! cuts to the first message that is safe to *begin* a conversation with.

use crate::agent::tool::{ContentBlock, Message};

/// Can this message legally start a conversation?
///
/// Only a plain user turn qualifies. An assistant message would leave the
/// transcript starting mid-exchange, and a user message carrying
/// `tool_result` blocks would orphan them from their `tool_use`.
fn is_safe_head(m: &Message) -> bool {
    m.role == "user"
        && !m
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
}

/// Drop the oldest messages until `history` holds at most `max_messages`,
/// cutting only at a boundary that leaves the transcript well-formed.
///
/// Returns the number of messages dropped. Compaction is best-effort and
/// deliberately conservative: if no safe cut point exists at or after the
/// ideal one, nothing is dropped. An over-long request that the provider
/// might still accept beats a malformed one it definitely will not.
///
/// `max_messages` of `0` or `usize::MAX` disables compaction.
pub fn compact_history(history: &mut Vec<Message>, max_messages: usize) -> usize {
    if max_messages == 0 || max_messages == usize::MAX || history.len() <= max_messages {
        return 0;
    }
    // Smallest cut that gets us under the limit.
    let ideal = history.len() - max_messages;
    // Walk forward to the first cut point that keeps the history valid.
    // The range excludes `history.len()`, so we can never empty the Vec.
    let Some(cut) = (ideal..history.len()).find(|&i| is_safe_head(&history[i])) else {
        tracing::debug!(
            len = history.len(),
            max_messages,
            "no safe compaction boundary; leaving history intact"
        );
        return 0;
    };
    if cut == 0 {
        return 0;
    }
    history.drain(..cut);
    tracing::debug!(
        dropped = cut,
        remaining = history.len(),
        "compacted history"
    );
    cut
}

/// Conservative local estimate used only for compaction. Providers tokenize
/// differently, but character volume is enough to stop a few huge messages
/// from bypassing the message-count limit.
pub fn estimate_tokens(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|m| 4 + m.text().chars().count().div_ceil(4))
        .sum()
}

/// Replace an old, complete prefix with a bounded textual summary when the
/// transcript exceeds `max_tokens`. Tool traffic is deliberately omitted from
/// the summary: it is implementation detail, can be huge, and may be unsafe
/// to replay as instructions.
pub fn compact_history_to_tokens(history: &mut Vec<Message>, max_tokens: usize) -> bool {
    if max_tokens == 0 || estimate_tokens(history) <= max_tokens {
        return false;
    }
    for cut in (1..history.len())
        .rev()
        .filter(|&i| is_safe_head(&history[i]))
    {
        let tail_tokens = estimate_tokens(&history[cut..]);
        if tail_tokens >= max_tokens {
            continue;
        }
        // Leave framing room and cap the textual summary to the available
        // budget. The result remains useful even for a very small window.
        let summary_cap = (max_tokens - tail_tokens)
            .saturating_mul(4)
            .saturating_sub(32);
        let mut compacted = Vec::with_capacity(history.len() - cut + 1);
        compacted.push(Message::user(summarize_prefix(
            &history[..cut],
            summary_cap,
        )));
        compacted.extend_from_slice(&history[cut..]);
        if estimate_tokens(&compacted) <= max_tokens {
            *history = compacted;
            return true;
        }
    }
    false
}

/// Replace an old, complete prefix with a bounded extractive summary.
///
/// This is deliberately extractive, not abstractive: compaction runs inside
/// a synchronous helper with no provider handle, so a model-written summary
/// is not possible here. A full LLM summarizer (background call, transcript
/// handoff) is deferred to Wave E; until then the durable SQLite transcript
/// remains the source of truth and this prefix only keeps the tail coherent.
fn summarize_prefix(messages: &[Message], cap: usize) -> String {
    /// Maximum sentences in the extractive prefix.
    const MAX_SENTENCES: usize = 12;
    const HEADER: &str =
        "[Earlier conversation summary — extractive; full transcript in durable memory]\n";
    let cap = cap.clamp(HEADER.len() + 32, 3_000);
    let mut out = String::from(HEADER);
    // Gather deduplicated sentences, user turns first: when the budget only
    // fits a few lines, what the human asked outranks assistant chatter.
    let mut seen = std::collections::HashSet::new();
    let mut lines: Vec<String> = Vec::new();
    for user_first in [true, false] {
        for message in messages {
            let is_user = message.role != "assistant";
            if is_user != user_first {
                continue;
            }
            let label = if message.role == "assistant" {
                "Assistant"
            } else {
                "User"
            };
            for sentence in split_sentences(&message.text()) {
                if seen.insert(sentence.to_lowercase()) {
                    lines.push(format!("{label}: {sentence}"));
                }
            }
        }
    }
    for line in lines.into_iter().take(MAX_SENTENCES) {
        if out.len() >= cap {
            break;
        }
        let room = cap.saturating_sub(out.len() + 1);
        if room == 0 {
            break;
        }
        out.push_str(&crate::util::truncate_bytes(&line, room));
        out.push('\n');
    }
    out
}

/// Split text into sentence-ish chunks on `.`/`!`/`?` and newline boundaries.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            let s = current.trim().to_string();
            if !s.is_empty() {
                sentences.push(s);
            }
            current.clear();
            continue;
        }
        current.push(c);
        if matches!(c, '.' | '!' | '?') {
            let boundary = match chars.peek() {
                None => true,
                Some(n) => n.is_whitespace(),
            };
            if boundary {
                let s = current.trim().to_string();
                if !s.is_empty() {
                    sentences.push(s);
                }
                current.clear();
            }
        }
    }
    let tail = current.trim().to_string();
    if !tail.is_empty() {
        sentences.push(tail);
    }
    sentences
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool::{ToolCall, ToolResult};
    use serde_json::json;

    fn tool_use_turn(id: &str) -> Message {
        Message::assistant(vec![ContentBlock::ToolUse(ToolCall {
            id: id.into(),
            name: "bash".into(),
            input: json!({"command": "ls"}),
        })])
    }

    /// One full exchange: user asks, assistant calls a tool, result comes
    /// back, assistant answers. Four messages, and the middle two must
    /// never be separated.
    fn exchange(n: usize) -> Vec<Message> {
        vec![
            Message::user(format!("q{n}")),
            tool_use_turn(&format!("call_{n}")),
            Message::tool_results(vec![ToolResult::ok(format!("call_{n}"), "out")]),
            Message::assistant_text(format!("a{n}")),
        ]
    }

    #[test]
    fn under_limit_is_untouched() {
        let mut h = exchange(1);
        assert_eq!(compact_history(&mut h, 50), 0);
        assert_eq!(h.len(), 4);
        assert_eq!(compact_history(&mut h, 0), 0, "0 disables compaction");
        assert_eq!(compact_history(&mut h, usize::MAX), 0);
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn drops_whole_exchanges_from_the_front() {
        let mut h: Vec<Message> = (1..=3).flat_map(exchange).collect();
        assert_eq!(h.len(), 12);
        let dropped = compact_history(&mut h, 6);
        assert_eq!(dropped, 8, "cuts forward to the head of exchange 3");
        assert_eq!(h.len(), 4);
        assert_eq!(h[0].text(), "q3");
    }

    #[test]
    fn never_orphans_a_tool_result() {
        // Every cut the function can make must leave a valid transcript:
        // head is a plain user turn, and every tool_result is directly
        // preceded by the assistant tool_use that produced it.
        for limit in 1..=12 {
            let mut h: Vec<Message> = (1..=3).flat_map(exchange).collect();
            compact_history(&mut h, limit);
            assert!(!h.is_empty(), "limit {limit} emptied the history");
            assert!(
                is_safe_head(&h[0]),
                "limit {limit} left a {:?} message at the head",
                h[0].role
            );
            for (i, m) in h.iter().enumerate() {
                let has_result = m
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                if has_result {
                    let prev = h.get(i.wrapping_sub(1));
                    assert!(
                        prev.is_some_and(|p| p
                            .content
                            .iter()
                            .any(|b| matches!(b, ContentBlock::ToolUse(_)))),
                        "limit {limit}: orphaned tool_result at index {i}"
                    );
                }
            }
        }
    }

    #[test]
    fn refuses_to_cut_when_no_safe_boundary_exists() {
        // A history that is entirely tool traffic has no legal head other
        // than index 0, so compaction must decline rather than corrupt it.
        let mut h = vec![
            Message::user("start"),
            tool_use_turn("c1"),
            Message::tool_results(vec![ToolResult::ok("c1", "out")]),
            tool_use_turn("c2"),
            Message::tool_results(vec![ToolResult::ok("c2", "out")]),
        ];
        let before = h.len();
        assert_eq!(compact_history(&mut h, 2), 0);
        assert_eq!(h.len(), before, "history left intact rather than broken");
    }

    #[test]
    fn compaction_is_idempotent() {
        let mut h: Vec<Message> = (1..=4).flat_map(exchange).collect();
        compact_history(&mut h, 5);
        let after_first = h.len();
        assert_eq!(compact_history(&mut h, 5), 0);
        assert_eq!(h.len(), after_first);
    }

    #[test]
    fn token_compaction_keeps_a_summary_and_safe_tail() {
        let mut h: Vec<Message> = (1..=3).flat_map(exchange).collect();
        // Budget must fit the summary header (~81 bytes) plus one exchange
        // tail: 57-token history needs 51 < budget <= 57 to compact at all.
        assert!(compact_history_to_tokens(&mut h, 56));
        assert!(h[0].text().contains("Earlier conversation summary"));
        assert!(h[0]
            .text()
            .contains("extractive; full transcript in durable memory"));
        assert!(estimate_tokens(&h) <= 56);
    }

    #[test]
    fn extractive_summary_dedupes_and_prefers_user_sentences() {
        let h = vec![
            Message::user("My editor is helix. My editor is helix."),
            Message::assistant_text("Noted, helix it is. My editor is helix."),
        ];
        let summary = summarize_prefix(&h, 3_000);
        assert!(
            summary.contains("extractive; full transcript in durable memory"),
            "marker: {summary}"
        );
        assert_eq!(
            summary.matches("My editor is helix").count(),
            1,
            "repeated sentence kept once: {summary}"
        );
        let user_pos = summary.find("User:").unwrap_or(usize::MAX);
        let assistant_pos = summary.find("Assistant:").unwrap_or(usize::MAX);
        assert!(
            user_pos < assistant_pos,
            "user sentences come first: {summary}"
        );
    }
}
