//! Integration tests for the SQLite memory store.

use zero_hermes::memory::Memory;

#[test]
fn notes_round_trip() {
    let m = Memory::in_memory().unwrap();
    m.write_note("k", "v").unwrap();
    assert_eq!(m.read_note("k").unwrap().as_deref(), Some("v"));
    assert!(m.read_note("missing").unwrap().is_none());
    assert!(m.delete_note("k").unwrap());
    assert!(m.read_note("k").unwrap().is_none());
}

#[test]
fn overwrite_note() {
    let m = Memory::in_memory().unwrap();
    m.write_note("k", "v1").unwrap();
    m.write_note("k", "v2").unwrap();
    assert_eq!(m.read_note("k").unwrap().as_deref(), Some("v2"));
}

#[test]
fn list_notes_sorted() {
    let m = Memory::in_memory().unwrap();
    m.write_note("b", "2").unwrap();
    m.write_note("a", "1").unwrap();
    m.write_note("c", "3").unwrap();
    let notes = m.list_notes().unwrap();
    assert_eq!(
        notes,
        vec![
            ("a".into(), "1".into()),
            ("b".into(), "2".into()),
            ("c".into(), "3".into()),
        ]
    );
}

#[test]
fn file_backed_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("memory.sqlite");
    {
        let m = Memory::open(Some(&path)).unwrap();
        m.write_note("k", "v").unwrap();
    }
    let m = Memory::open(Some(&path)).unwrap();
    assert_eq!(m.read_note("k").unwrap().as_deref(), Some("v"));
}

#[test]
fn sessions_crud() {
    let m = Memory::in_memory().unwrap();
    m.upsert_session("s1", "telegram", "42").unwrap();
    m.upsert_session("s2", "cli", "1").unwrap();
    let list = m.list_sessions().unwrap();
    assert_eq!(list.len(), 2);
    m.upsert_session("s1", "telegram", "99").unwrap();
    let list = m.list_sessions().unwrap();
    let s1 = list.iter().find(|s| s.id == "s1").unwrap();
    assert_eq!(s1.chat_id, "99");
}
