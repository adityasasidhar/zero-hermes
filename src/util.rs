//! Small shared utilities.

/// Truncate `s` to at most `max_bytes` bytes without splitting a UTF-8
/// codepoint, appending `…` when truncation occurred. Safe for arbitrary
/// text (notes, error bodies, etc.); the per-module byte-slicing
/// `truncate` helpers used to panic on multi-byte characters.
pub fn truncate_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = String::with_capacity(end + 4);
    out.push_str(&s[..end]);
    out.push('\u{2026}');
    out
}

/// Truncate `s` to at most `max_chars` characters, appending `…` when
/// truncation occurred. Character-based counterpart to [`truncate_bytes`]
/// for budgets expressed in characters (prompt-section caps).
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push('\u{2026}');
    out
}

/// Expand a leading `~` or `~/` in a path against the user's home directory.
///
/// Config files are written by hand and by `init-config`, and `~` in a TOML
/// string is just a character — nothing expands it on the way in. Without
/// this, `path = "~/.local/share/..."` creates a *literal* `~` directory
/// under the process CWD. Anything that is not a leading tilde is returned
/// unchanged.
pub fn expand_tilde(path: &std::path::Path) -> std::path::PathBuf {
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    if s == "~" {
        return dirs::home_dir().unwrap_or_else(|| path.to_path_buf());
    }
    let Some(rest) = s.strip_prefix("~/") else {
        return path.to_path_buf();
    };
    match dirs::home_dir() {
        Some(home) => home.join(rest),
        None => path.to_path_buf(),
    }
}

/// Split `s` into chunks of at most `max_chars` characters, preferring to
/// break on a newline and falling back to a whitespace boundary before
/// finally cutting mid-run.
///
/// Counting is by `char`, not bytes, because the limits this exists for
/// (Telegram's 4096-character message cap) are expressed in characters.
/// Returns a single empty chunk for empty input so callers always send
/// something.
pub fn chunk_text(s: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 {
        return vec![s.to_string()];
    }
    if s.chars().count() <= max_chars {
        return vec![s.to_string()];
    }

    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        if rest.chars().count() <= max_chars {
            out.push(rest.to_string());
            break;
        }
        // Byte index just past the max_chars'th character.
        let hard_end = rest
            .char_indices()
            .nth(max_chars)
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let window = &rest[..hard_end];
        // Prefer a newline break, then any whitespace, then the hard cut.
        let split = window
            .rfind('\n')
            .map(|i| i + 1)
            .or_else(|| window.rfind(char::is_whitespace).map(|i| i + 1))
            .unwrap_or(hard_end);
        let split = if split == 0 { hard_end } else { split };
        out.push(rest[..split].to_string());
        rest = &rest[split..];
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Generate `n_bytes` of OS entropy rendered as lowercase hex.
///
/// Used for the web UI's CSRF token. Falls back to a clock-derived value if
/// the OS RNG is unavailable, which is weaker but still unguessable by an
/// off-host attacker who cannot observe the process start time.
pub fn random_hex(n_bytes: usize) -> String {
    let mut buf = vec![0u8; n_bytes];
    if getrandom::getrandom(&mut buf).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((nanos >> ((i % 16) * 8)) as u8) ^ (i as u8);
        }
    }
    let mut out = String::with_capacity(n_bytes * 2);
    for b in buf {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Load `KEY=VALUE` pairs from a dotenv-style file into the process
/// environment, without overwriting variables that are already set.
///
/// Deliberately tiny and dependency-free: `#`-comments, blank lines, an
/// optional `export ` prefix, and single- or double-quoted values. Secrets
/// belong in `.env` (gitignored) rather than in `zero_hermes.toml`, which
/// people check in.
///
/// Returns the names of the variables it set. A missing file is not an
/// error — it returns an empty list.
pub fn load_dotenv(path: &std::path::Path) -> Vec<String> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut set = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        // Strip surrounding quotes, then trailing inline comments on
        // unquoted values.
        let value = value.trim();
        let value = match (value.strip_prefix('"'), value.strip_prefix('\'')) {
            (Some(rest), _) => rest.strip_suffix('"').unwrap_or(rest).to_string(),
            (_, Some(rest)) => rest.strip_suffix('\'').unwrap_or(rest).to_string(),
            _ => value
                .split_once(" #")
                .map(|(v, _)| v)
                .unwrap_or(value)
                .trim()
                .to_string(),
        };
        // Real environment wins, so `FOO=bar zero-hermes ...` still works.
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, &value);
            set.push(key.to_string());
        }
    }
    set
}

/// Substitute `${VAR}` and `$VAR` references from the environment.
///
/// An unset variable expands to the empty string, matching shell
/// behaviour. Use it on config values that should not contain a literal
/// secret — `api_key = "${MINIMAX_API_KEY}"`.
pub fn expand_env_vars(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'$' {
                i += 1;
            }
            out.push_str(&s[start..i]);
            continue;
        }
        // `${NAME}`
        if let Some(rest) = s[i..].strip_prefix("${") {
            if let Some(end) = rest.find('}') {
                let name = &rest[..end];
                out.push_str(&std::env::var(name).unwrap_or_default());
                i += 2 + end + 1;
                continue;
            }
        }
        // `$NAME`
        let name_start = i + 1;
        let mut name_end = name_start;
        while name_end < bytes.len()
            && (bytes[name_end].is_ascii_alphanumeric() || bytes[name_end] == b'_')
        {
            name_end += 1;
        }
        if name_end > name_start {
            out.push_str(&std::env::var(&s[name_start..name_end]).unwrap_or_default());
            i = name_end;
        } else {
            out.push('$');
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_string_unchanged() {
        assert_eq!(truncate_bytes("hello", 10), "hello");
    }

    #[test]
    fn exact_boundary_no_ellipsis() {
        assert_eq!(truncate_bytes("hello", 5), "hello");
    }

    #[test]
    fn ascii_truncation() {
        assert_eq!(truncate_bytes("hello world", 5), "hello\u{2026}");
    }

    #[test]
    fn multibyte_safe_truncation() {
        // "héllo" — 'h'(1) + 'é'(2) + 'l'(1) + 'l'(1) + 'o'(1) = 6 bytes
        let s = "héllo";
        // max=2 lands in the middle of 'é' (bytes 1..3). Without
        // is_char_boundary, `&s[..2]` would panic. Snap back to 1.
        assert_eq!(truncate_bytes(s, 2), "h\u{2026}");
        // max=3 is the boundary right after 'é': result is "hé".
        assert_eq!(truncate_bytes(s, 3), "hé\u{2026}");
        // max=4 includes 'h' + 'é' + 'l'.
        assert_eq!(truncate_bytes(s, 4), "hél\u{2026}");
        // max=5 includes 'h' + 'é' + 'l' + 'l'.
        assert_eq!(truncate_bytes(s, 5), "héll\u{2026}");
    }

    #[test]
    fn empty_string() {
        assert_eq!(truncate_bytes("", 5), "");
    }

    #[test]
    fn truncate_chars_counts_characters_not_bytes() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("hello", 5), "hello");
        assert_eq!(truncate_chars("hello world", 5), "hello\u{2026}");
        // Six multi-byte chars capped at four: no split codepoint, no panic.
        assert_eq!(truncate_chars("é".repeat(6).as_str(), 4), "éééé\u{2026}");
        assert_eq!(truncate_chars("", 5), "");
    }

    #[test]
    fn expand_tilde_rewrites_leading_home() {
        let home = dirs::home_dir().expect("home dir");
        let p = expand_tilde(std::path::Path::new("~/.local/share/x.sqlite"));
        assert_eq!(p, home.join(".local/share/x.sqlite"));
        assert_eq!(expand_tilde(std::path::Path::new("~")), home);
    }

    #[test]
    fn expand_tilde_leaves_other_paths_alone() {
        for raw in ["/abs/path", "relative/path", "./x", "not~here", "~user/x"] {
            assert_eq!(
                expand_tilde(std::path::Path::new(raw)),
                std::path::PathBuf::from(raw),
                "{raw} should be untouched"
            );
        }
    }

    #[test]
    fn chunk_text_short_is_single_chunk() {
        assert_eq!(chunk_text("hello", 10), vec!["hello".to_string()]);
        assert_eq!(chunk_text("", 10), vec![String::new()]);
    }

    #[test]
    fn chunk_text_prefers_newline_boundary() {
        let s = "aaaa\nbbbb\ncccc";
        let chunks = chunk_text(s, 10);
        assert_eq!(chunks, vec!["aaaa\nbbbb\n".to_string(), "cccc".to_string()]);
        assert_eq!(chunks.concat(), s);
    }

    #[test]
    fn chunk_text_falls_back_to_hard_cut() {
        // No whitespace anywhere: must still split, and never exceed the cap.
        let s = "x".repeat(25);
        let chunks = chunk_text(&s, 10);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| c.chars().count() <= 10));
        assert_eq!(chunks.concat(), s);
    }

    #[test]
    fn chunk_text_respects_char_not_byte_limits() {
        // 12 multi-byte chars: a byte-based split would produce 3+ chunks
        // and could slice a codepoint.
        let s = "é".repeat(12);
        let chunks = chunk_text(&s, 10);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|c| c.chars().count() <= 10));
        assert_eq!(chunks.concat(), s);
    }

    #[test]
    fn random_hex_is_right_length_and_varies() {
        let a = random_hex(16);
        let b = random_hex(16);
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "two tokens should not collide");
    }

    #[test]
    fn dotenv_parses_the_usual_shapes() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(".env");
        std::fs::write(
            &p,
            "# a comment\n\
             \n\
             ZH_TEST_PLAIN=hello\n\
             export ZH_TEST_EXPORTED=world\n\
             ZH_TEST_DQ=\"quoted value\"\n\
             ZH_TEST_SQ='single'\n\
             ZH_TEST_COMMENT=value # trailing\n\
             ZH_TEST_EQUALS=sk-cp-a=b=c\n",
        )
        .unwrap();
        let set = load_dotenv(&p);
        assert!(set.contains(&"ZH_TEST_PLAIN".to_string()));
        assert_eq!(std::env::var("ZH_TEST_PLAIN").unwrap(), "hello");
        assert_eq!(std::env::var("ZH_TEST_EXPORTED").unwrap(), "world");
        assert_eq!(std::env::var("ZH_TEST_DQ").unwrap(), "quoted value");
        assert_eq!(std::env::var("ZH_TEST_SQ").unwrap(), "single");
        assert_eq!(std::env::var("ZH_TEST_COMMENT").unwrap(), "value");
        // API keys contain `=`; only the first one separates key from value.
        assert_eq!(std::env::var("ZH_TEST_EQUALS").unwrap(), "sk-cp-a=b=c");
    }

    #[test]
    fn dotenv_does_not_clobber_the_real_environment() {
        std::env::set_var("ZH_TEST_PRESET", "from-shell");
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(".env");
        std::fs::write(&p, "ZH_TEST_PRESET=from-file\n").unwrap();
        let set = load_dotenv(&p);
        assert!(!set.contains(&"ZH_TEST_PRESET".to_string()));
        assert_eq!(std::env::var("ZH_TEST_PRESET").unwrap(), "from-shell");
    }

    #[test]
    fn dotenv_missing_file_is_not_an_error() {
        assert!(load_dotenv(std::path::Path::new("/nope/.env")).is_empty());
    }

    #[test]
    fn expand_env_handles_both_syntaxes() {
        std::env::set_var("ZH_TEST_KEY", "sk-secret");
        assert_eq!(expand_env_vars("${ZH_TEST_KEY}"), "sk-secret");
        assert_eq!(expand_env_vars("$ZH_TEST_KEY"), "sk-secret");
        assert_eq!(
            expand_env_vars("Bearer ${ZH_TEST_KEY}!"),
            "Bearer sk-secret!"
        );
        // Unset expands empty; a bare `$` and plain text are untouched.
        assert_eq!(expand_env_vars("${ZH_TEST_UNSET_XYZ}"), "");
        assert_eq!(expand_env_vars("literal-key"), "literal-key");
        assert_eq!(expand_env_vars("costs $ 5"), "costs $ 5");
        assert_eq!(expand_env_vars("${unclosed"), "${unclosed");
    }
}
