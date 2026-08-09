//! Integration tests for the Telegram message parser.

use zero_hermes::channels::telegram::TelegramChannel;
use zero_hermes::channels::{parse_command, strip_bot_mention, Channel};

#[test]
fn parse_simple_command() {
    let (cmd, args) = parse_command("/hello");
    assert_eq!(cmd.as_deref(), Some("hello"));
    assert!(args.is_none());
}

#[test]
fn parse_command_with_args() {
    let (cmd, args) = parse_command("/echo hello world");
    assert_eq!(cmd.as_deref(), Some("echo"));
    assert_eq!(args.as_deref(), Some("hello world"));
}

#[test]
fn parse_command_with_mention() {
    let (cmd, args) = parse_command("/run@my_bot please do it");
    assert_eq!(cmd.as_deref(), Some("run@my_bot"));
    assert_eq!(strip_bot_mention(cmd.as_deref().unwrap()), "run");
    assert_eq!(args.as_deref(), Some("please do it"));
}

#[test]
fn parse_no_command() {
    let (cmd, args) = parse_command("just chatting");
    assert!(cmd.is_none());
    assert!(args.is_none());
}

#[test]
fn channel_name_is_telegram() {
    let ch = TelegramChannel::new(zero_hermes::channels::telegram::TelegramConfig {
        token: "abc".into(),
        poll_timeout: 30,
        allowed_chats: Vec::new(),
        allowed_commands: Vec::new(),
    });
    assert_eq!(ch.name(), "telegram");
}
