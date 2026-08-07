//! JSONC comment-preservation spike (TODO §1, P0).
//!
//! Validates the jsonc-parser CST approach against a realistic opencode.jsonc
//! fixture. Run: `cargo run --example jsonc_spike`.

use anyhow::Result;
use jsonc_parser::cst::{CstInputValue, CstRootNode};
use jsonc_parser::ParseOptions;

const FIXTURE: &str = r#"// User-wide OpenCode config (hand-written).
// DO NOT commit secrets here.
{
  "$schema": "https://opencode.ai/config.json",   // schema ref comment
  "model": "anthropic/claude-3-5-sonnet", // primary model
  "theme": "dark",
  /* block comment:
     multi-line */
  "provider": {
    "anthropic": {
      "options": {
        "baseURL": "https://api.anthropic.com",
        "headers": { "x-custom": "keep-me" },
      },
    },
    "openai": {
      // trailing comma below, please keep it
      "options": { "baseURL": "https://api.openai.com" },
    },
  },
  "autoshare": false, /*trailing block*/
}
"#;

fn main() -> Result<()> {
    let mut failures = 0;

    // Test 1: parse + no-op serialize must be byte-identical.
    let root = CstRootNode::parse(FIXTURE, &ParseOptions::default())
        .map_err(|e| anyhow::anyhow!("parse failed: {e:?}"))?;
    let serialized = root.to_string();
    check(
        "no-op round-trip byte-identical",
        serialized == FIXTURE,
        &mut failures,
    );

    // Test 2: mutate one key, everything else must be preserved.
    let root2 = CstRootNode::parse(FIXTURE, &ParseOptions::default())
        .map_err(|e| anyhow::anyhow!("parse failed: {e:?}"))?;
    let root_obj = root2.object_value_or_set();
    match root_obj.get("model") {
        Some(prop) => prop.set_value(CstInputValue::String("openai/gpt-4o".into())),
        None => bail_prop_missing()?,
    }
    let edited = root2.to_string();

    check("edited differs", edited != FIXTURE, &mut failures);
    for token in [
        "// User-wide OpenCode config",
        "// schema ref comment",
        "// primary model",
        "/* block comment:",
        "multi-line */",
        "// trailing comma below, please keep it",
        "/*trailing block*/",
        "\"$schema\"",
        "\"x-custom\": \"keep-me\"",
        "\"openai/gpt-4o\"",
    ] {
        check(
            &format!("retains: {token}"),
            edited.contains(token),
            &mut failures,
        );
    }
    // Trailing commas must still exist (count preserved).
    let trailing_commas = |s: &str| s.matches(",\n").count() + s.matches(",\r").count();
    check(
        "trailing comma count preserved",
        trailing_commas(&edited) == trailing_commas(FIXTURE),
        &mut failures,
    );
    // Old value must be gone.
    check(
        "old model value replaced",
        !edited.contains("claude-3-5-sonnet"),
        &mut failures,
    );
    // Surgical-edit sanity: only the value delta should change (no reformatting).
    let expected_delta =
        ("openai/gpt-4o".len() as i64) - ("anthropic/claude-3-5-sonnet".len() as i64);
    let actual_delta = edited.len() as i64 - FIXTURE.len() as i64;
    check(
        &format!("edit is surgical (len delta {actual_delta} == {expected_delta})"),
        actual_delta == expected_delta,
        &mut failures,
    );

    if failures == 0 {
        println!("ALL SPIKE CHECKS PASSED");
        Ok(())
    } else {
        anyhow::bail!("{failures} spike check(s) FAILED");
    }
}

fn bail_prop_missing() -> Result<()> {
    anyhow::bail!("property 'model' not found in fixture (fixture bug)")
}

fn check(name: &str, ok: bool, failures: &mut u32) {
    if ok {
        println!("PASS  {name}");
    } else {
        eprintln!("FAIL  {name}");
        *failures += 1;
    }
}
