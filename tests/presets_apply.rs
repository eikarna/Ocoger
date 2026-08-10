//! End-to-end preset apply: seeds agents + presets.jsonc, drives the MVU
//! actions the same way `event_handler` would, asserts disk state after `s`.
//! Covers ROADMAP Phase 4 acceptance for the Presets feature.

use ocoger::core::agent_parser::load_agent;
use ocoger::ui::app::{Action, App, Mode};
use std::fs;
use tempfile::TempDir;

fn agent_src(name: &str, model: &str) -> String {
    format!(
        r#"---
# {name}
model: {model}   # primary
temperature: 0.2
top_k: 50
---
body text kept verbatim for {name}
"#
    )
}

fn presets_src() -> &'static str {
    // JSONC with comments to verify parse tolerates them.
    r#"{
  // capture from deep-work runs
  "presets": [
    {
      "name": "deep-work",
      "description": "long-form reasoning",
      "model": "anthropic/claude-sonnet-4",
      "temperature": 0.3,
      "top_p": 0.95,
      "reasoning_effort": "high"
    },
    {
      "name": "fast-draft",
      "model": "openai/gpt-5-mini",
      "temperature": 0.7,
      "top_k": 40,
      "reasoning_effort": "low"
    }
  ]
}"#
}

fn seed(root: &std::path::Path) {
    let agents_dir = root.join(".opencode/agents");
    let ocoger_dir = root.join(".ocoger");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(&ocoger_dir).unwrap();
    fs::write(
        agents_dir.join("a.md"),
        agent_src("a", "anthropic/claude-3-5-sonnet"),
    )
    .unwrap();
    fs::write(agents_dir.join("b.md"), agent_src("b", "openai/gpt-4o")).unwrap();
    fs::write(
        agents_dir.join("c.md"),
        agent_src("c", "anthropic/claude-haiku"),
    )
    .unwrap();
    fs::write(ocoger_dir.join("presets.jsonc"), presets_src()).unwrap();
}

#[test]
fn preset_apply_to_selected_then_save_flips_only_selected() {
    let tmp = TempDir::new().unwrap();
    seed(tmp.path());

    let paths = ocoger::core::agent_scanner::scan_agents(tmp.path()).unwrap();
    let agents: Vec<_> = paths.iter().map(|p| load_agent(p).unwrap()).collect();
    assert_eq!(agents.len(), 3);

    let mut app = App::new(agents, tmp.path().to_path_buf());
    app.mode = Mode::List;
    assert_eq!(
        app.presets.len(),
        2,
        "presets file loaded: {:?}",
        app.presets.len()
    );

    // Select agents a + c only (skip b).
    app.agents[0].is_selected = true;
    app.agents[2].is_selected = true;

    app.update(Action::OpenPresets);
    assert_eq!(app.mode, Mode::Preset);

    // Filter to "deep-work".
    for c in "deep".chars() {
        app.update(Action::PresetInput(c));
    }
    assert_eq!(app.preset_items.len(), 1);
    assert_eq!(app.preset_items[0].name, "deep-work");

    app.update(Action::PresetAccept);
    assert_eq!(app.mode, Mode::List);

    app.update(Action::Save);

    let a = fs::read_to_string(tmp.path().join(".opencode/agents/a.md")).unwrap();
    let b = fs::read_to_string(tmp.path().join(".opencode/agents/b.md")).unwrap();
    let c = fs::read_to_string(tmp.path().join(".opencode/agents/c.md")).unwrap();

    assert!(a.contains("model: anthropic/claude-sonnet-4   # primary"));
    assert!(a.contains("top_p: 0.95"), "top_p appended from preset");
    assert!(a.contains("reasoning_effort: high"), "new key appended");
    assert!(a.contains("# a"), "header comment preserved");
    assert!(a.contains("body text kept verbatim for a"));

    assert!(
        b.contains("model: openai/gpt-4o   # primary"),
        "unselected agent untouched: {b}"
    );

    assert!(c.contains("model: anthropic/claude-sonnet-4   # primary"));
    assert!(c.contains("reasoning_effort: high"));

    // Ensure the original YAML comment on the `model:` line survived.
    assert!(a.contains("# primary"));
    assert!(c.contains("# primary"));
}

#[test]
fn preset_capture_from_selection_then_apply_all_confirm() {
    let tmp = TempDir::new().unwrap();
    seed(tmp.path());

    let paths = ocoger::core::agent_scanner::scan_agents(tmp.path()).unwrap();
    let agents: Vec<_> = paths.iter().map(|p| load_agent(p).unwrap()).collect();
    let mut app = App::new(agents, tmp.path().to_path_buf());
    app.mode = Mode::List;
    assert_eq!(app.presets.len(), 2);

    // Select agent b only, capture it as a preset.
    app.agents[1].is_selected = true;
    app.update(Action::OpenPresets);
    app.update(Action::PresetNewStart);
    assert_eq!(app.mode, Mode::PresetNameNew);

    for c in "agent-b-snapshot".chars() {
        app.update(Action::PresetInput(c));
    }
    app.update(Action::PresetSaveNew);
    assert_eq!(app.mode, Mode::Preset);

    // New preset persisted to disk and visible in memory.
    let reloaded = ocoger::core::presets::Presets::load(tmp.path()).unwrap();
    let snapshot = reloaded.get("agent-b-snapshot").expect("saved");
    assert_eq!(snapshot.model, "openai/gpt-4o");
    assert_eq!(snapshot.temperature, Some(0.2));
    assert_eq!(snapshot.top_k, Some(50));

    // Now apply it to all agents via Shift+A -> y.
    for c in "agent-b".chars() {
        app.update(Action::PresetInput(c));
    }
    app.update(Action::PresetApplyAllStart);
    assert_eq!(app.mode, Mode::PresetConfirmAll);

    app.update(Action::ConfirmAllYes);
    assert_eq!(app.mode, Mode::List);

    app.update(Action::Save);

    for n in ["a", "b", "c"] {
        let body = fs::read_to_string(tmp.path().join(format!(".opencode/agents/{n}.md"))).unwrap();
        assert!(
            body.contains("model: openai/gpt-4o"),
            "agent {n} must flip: {body}"
        );
    }
}

#[test]
fn cancel_in_apply_all_confirm_is_a_noop() {
    let tmp = TempDir::new().unwrap();
    seed(tmp.path());
    let paths = ocoger::core::agent_scanner::scan_agents(tmp.path()).unwrap();
    let agents: Vec<_> = paths.iter().map(|p| load_agent(p).unwrap()).collect();
    let mut app = App::new(agents, tmp.path().to_path_buf());
    app.mode = Mode::List;

    app.agents[0].is_selected = true;
    app.update(Action::OpenPresets);
    app.update(Action::PresetApplyAllStart);
    assert_eq!(app.mode, Mode::PresetConfirmAll);
    app.update(Action::ConfirmAllNo);

    assert_eq!(app.mode, Mode::Preset);
    // Nothing marked dirty.
    assert_eq!(app.dirty_count(), 0, "cancel must not stage any edits");
}

#[test]
fn delete_preset_from_modal_persists_to_disk() {
    let tmp = TempDir::new().unwrap();
    seed(tmp.path());
    let paths = ocoger::core::agent_scanner::scan_agents(tmp.path()).unwrap();
    let agents: Vec<_> = paths.iter().map(|p| load_agent(p).unwrap()).collect();
    let mut app = App::new(agents, tmp.path().to_path_buf());
    app.mode = Mode::List;
    assert_eq!(app.presets.len(), 2);

    app.update(Action::OpenPresets);
    // Filter "fast-draft" then delete.
    for c in "fast".chars() {
        app.update(Action::PresetInput(c));
    }
    app.update(Action::PresetDelete);

    let reloaded = ocoger::core::presets::Presets::load(tmp.path()).unwrap();
    assert!(
        !reloaded.items.iter().any(|p| p.name == "fast-draft"),
        "delete persisted"
    );
    assert_eq!(reloaded.items.len(), 1);
    assert_eq!(reloaded.items[0].name, "deep-work");
}
