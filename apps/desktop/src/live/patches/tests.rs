use super::*;
use caelum_core::application::{Entry, EntryKind};
use rune_kit::diff::{LineRange, LineRef, Side};
use serde_json::json;

fn fixture() -> (Live, Key) {
    let mut state = crate::evaluation::design();
    crate::evaluation::scene(&mut state, "patch-short");
    let mut live = state.live.take().unwrap();
    let key = (0, "opencode-tool-patch".to_owned());
    live.visible_entries.insert(key.clone());
    (live, key)
}

fn entry<'a>(live: &'a mut Live, key: &Key) -> &'a mut Entry {
    live.app.sessions[key.0]
        .conversation
        .entries
        .iter_mut()
        .find(|e| e.id == key.1)
        .unwrap()
}

#[test]
fn unchanged_patch_inputs_preserve_native_state_through_output_and_status_updates() {
    let (mut live, key) = fixture();
    assert!(live.sync_patches());
    let selection = Some(LineRange::line(0, LineRef::new(Side::New, 1)));
    let document = live.patches.get_mut(&key).unwrap();
    document.diff.set_selection(selection);
    document.diff.set_collapsed(0, true);
    let e = entry(&mut live, &key);
    e.output = "large streaming output\n".repeat(8192);
    e.error = "tool failed".into();
    e.status = "failed".into();
    e.metadata = json!({"durationMs":4000,"ended":5000});
    assert!(!live.sync_patches());
    assert_eq!(live.patches[&key].diff.selection(), selection);
    assert!(live.patches[&key].diff.collapsed(0));

    // A history prepend invalidates the cached position, not the entry's identity
    // or its native selection/disclosure state.
    let mut older = entry(&mut live, &key).clone();
    older.id = "older-patch".into();
    live.app.sessions[0].conversation.entries.insert(0, older);
    assert!(!live.sync_patches());
    assert_eq!(live.patches[&key].diff.selection(), selection);
    assert!(live.patches[&key].diff.collapsed(0));

    // Same-length replacements must invalidate; no length/pointer heuristics.
    let original = entry(&mut live, &key).input.clone();
    entry(&mut live, &key).input = original.replace("session.rs", "another.rs");
    assert_eq!(entry(&mut live, &key).input.len(), original.len());
    assert!(live.sync_patches());
    assert_eq!(live.patches[&key].diff.files()[0].path(), "src/another.rs");
    entry(&mut live, &key).input = "{}".into();
    assert!(live.sync_patches());
    assert!(!live.patches.contains_key(&key));
    assert!(!live.sync_patches());
    entry(&mut live, &key).input = original;
    assert!(live.sync_patches());
}

#[test]
fn collapse_navigation_and_history_replacement_keep_inputs_owned_by_their_session() {
    let (mut live, key) = fixture();
    assert!(live.sync_patches());
    live.collapsed_patches.insert(key.clone());
    entry(&mut live, &key).input = json!({"path":"new.rs","patch":"-old\n+new\n"}).to_string();
    assert!(!live.sync_patches());
    assert_eq!(live.patches[&key].diff.files()[0].path(), "src/session.rs");
    live.collapsed_patches.remove(&key);
    assert!(live.sync_patches());
    assert_eq!(live.patches[&key].diff.files()[0].path(), "new.rs");

    let mut other = entry(&mut live, &key).clone();
    other.input = json!({"path":"other.rs","patch":"-a\n+b\n"}).to_string();
    live.app.sessions[1].conversation.entries = vec![other];
    let second = (1, key.1.clone());
    live.visible_entries = HashSet::from([second.clone()]);
    assert!(live.sync_patches());
    assert_eq!(live.patches[&second].diff.files()[0].path(), "other.rs");
    assert_eq!(live.patches[&key].diff.files()[0].path(), "new.rs");
    assert!(!live.patch_inputs.values.contains_key(&key));
    // Entry identity can survive a provider history replacement with a new kind.
    entry(&mut live, &second).kind = EntryKind::Assistant;
    entry(&mut live, &second).name.clear();
    assert!(live.sync_patches());
    assert!(!live.patches.contains_key(&second));
    live.visible_entries = HashSet::from([key.clone()]);
    assert!(!live.sync_patches());
    live.app.sessions[0].conversation.entries.clear();
    assert!(live.sync_patches());
    assert!(!live.patches.contains_key(&key));
}

#[test]
fn input_cache_bounds_entries_and_bytes_and_reclaims_offscreen_content() {
    let (mut live, key) = fixture();
    let e = entry(&mut live, &key);
    let mut cache = Inputs::default();
    e.input = "x".repeat(MAX_BYTES / 2);
    for n in 0..MAX_ENTRIES * 2 {
        cache.remember(&(n, "tool".into()), n, e);
        assert!(cache.bytes <= MAX_BYTES);
    }
    assert_eq!(cache.values.len(), 1);
    cache.retain(&HashSet::new());
    assert_eq!(cache.bytes, 0);
    e.input = "{}".into();
    for n in 0..MAX_ENTRIES * 2 {
        cache.remember(&(n, "tool".into()), n, e);
    }
    assert_eq!(cache.values.len(), MAX_ENTRIES);
    // Replacing an existing entry with an oversized input cannot leave a stale hit.
    e.input = "x".repeat(MAX_BYTES);
    cache.remember(&(0, "tool".into()), 0, e);
    e.input = "{}".into();
    assert!(!cache.unchanged(&(0, "tool".into()), 0, e));
    cache.retain(&HashSet::new());
    assert_eq!(cache.bytes, 0);
}

#[test]
fn tool_identity_changes_invalidate_previews_with_identical_input() {
    let (mut live, key) = fixture();
    entry(&mut live, &key).name = "shell".into();
    assert!(!live.sync_patches());
    entry(&mut live, &key).name = "edit".into();
    assert!(live.sync_patches());
    assert!(live.patches.contains_key(&key));
    entry(&mut live, &key).kind = EntryKind::Reasoning;
    assert!(live.sync_patches());
    assert!(!live.patches.contains_key(&key));
    entry(&mut live, &key).kind = EntryKind::Tool;
    assert!(live.sync_patches());
}
