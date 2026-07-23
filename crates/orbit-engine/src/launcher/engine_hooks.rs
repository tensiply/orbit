use orbit_core::engine_hook::{EngineHookCatalog, EngineHookState, expand_home};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Build the `hooks` section of a Claude Code settings.json from enabled catalog entries.
/// Returns `None` if no hooks are enabled (caller skips writing the file).
pub fn build_settings(state: &EngineHookState, catalog: &[EngineHookCatalog]) -> Option<Value> {
    // Group by (event, matcher) so hooks with different matchers get separate group objects.
    let mut by_key: BTreeMap<(String, Option<String>), Vec<Value>> = BTreeMap::new();

    for entry in catalog {
        if !state.is_enabled(&entry.name) {
            continue;
        }
        for ev in &entry.events {
            let cmd = expand_home(&ev.command);
            by_key
                .entry((ev.event.clone(), ev.matcher.clone()))
                .or_default()
                .push(json!({"type": "command", "command": cmd}));
        }
    }

    if by_key.is_empty() {
        return None;
    }

    // Claude Code format: { "EventName": [{"matcher": "...", "hooks": [...]}, ...], ... }
    let mut by_event: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for ((event, matcher), hook_list) in by_key {
        let group = match matcher {
            Some(m) => json!({"matcher": m, "hooks": hook_list}),
            None => json!({"hooks": hook_list}),
        };
        by_event.entry(event).or_default().push(group);
    }

    let hooks_val: serde_json::Map<String, Value> = by_event
        .into_iter()
        .map(|(event, groups)| (event, serde_json::json!(groups)))
        .collect();

    Some(json!({"hooks": hooks_val}))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_core::engine_hook::{EngineHookCatalog, EngineHookEventDef, EngineHookState};

    fn make_catalog() -> Vec<EngineHookCatalog> {
        vec![EngineHookCatalog {
            name: "session-logging".into(),
            description: "test".into(),
            category: "governance".into(),
            events: vec![EngineHookEventDef {
                event: "Stop".into(),
                command: "/tmp/on-stop.sh".into(),
                matcher: None,
                is_async: false,
            }],
            requires_binary: None,
            scripts: vec![],
        }]
    }

    #[test]
    fn returns_none_when_nothing_enabled() {
        let state = EngineHookState::default();
        let catalog = make_catalog();
        assert!(build_settings(&state, &catalog).is_none());
    }

    #[test]
    fn builds_correct_structure_when_enabled() {
        let mut state = EngineHookState::default();
        state.enable("session-logging");
        let catalog = make_catalog();
        let val = build_settings(&state, &catalog).unwrap();

        let stop = &val["hooks"]["Stop"];
        assert!(stop.is_array());
        let hooks = &stop[0]["hooks"];
        assert!(hooks.is_array());
        assert_eq!(hooks[0]["type"], "command");
        assert_eq!(hooks[0]["command"], "/tmp/on-stop.sh");
        // no matcher → group should not have "matcher" key
        assert!(stop[0].get("matcher").is_none());
    }

    #[test]
    fn matcher_included_in_group_when_set() {
        let catalog = vec![EngineHookCatalog {
            name: "bash-guard".into(),
            description: "test".into(),
            category: "security".into(),
            events: vec![EngineHookEventDef {
                event: "PreToolUse".into(),
                command: "/tmp/guard.sh".into(),
                matcher: Some("Bash".into()),
                is_async: false,
            }],
            requires_binary: None,
            scripts: vec![],
        }];
        let mut state = EngineHookState::default();
        state.enable("bash-guard");
        let val = build_settings(&state, &catalog).unwrap();

        let pre = &val["hooks"]["PreToolUse"];
        assert!(pre.is_array());
        assert_eq!(pre[0]["matcher"], "Bash");
        let hooks = &pre[0]["hooks"];
        assert!(hooks.is_array());
        assert_eq!(hooks[0]["command"], "/tmp/guard.sh");
    }
}
