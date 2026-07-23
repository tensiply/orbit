use orbit_core::engine_hook::{EngineHookCatalog, EngineHookState, expand_home};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Build the `hooks` section of a Claude Code settings.json from enabled catalog entries.
/// Returns `None` if no hooks are enabled (caller skips writing the file).
pub fn build_settings(state: &EngineHookState, catalog: &[EngineHookCatalog]) -> Option<Value> {
    let mut by_event: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    for entry in catalog {
        if !state.is_enabled(&entry.name) {
            continue;
        }
        for ev in &entry.events {
            let cmd = expand_home(&ev.command);
            by_event
                .entry(ev.event.clone())
                .or_default()
                .push(json!({"type": "command", "command": cmd}));
        }
    }

    if by_event.is_empty() {
        return None;
    }

    // Claude Code format: { "EventName": [{"hooks": [...]}], ... }
    let hooks_val: serde_json::Map<String, Value> = by_event
        .into_iter()
        .map(|(event, hook_list)| (event, json!([{"hooks": hook_list}])))
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
    }
}
