//! Adding / removing Escape's hooks in Claude Code's `~/.claude/settings.json`.
//!
//! Only entries whose command contains our endpoint are touched; everything else
//! in the file is preserved (key order included, via serde_json `preserve_order`).

use serde_json::{json, Map, Value};

use super::http::PATH;

pub const PORT: u16 = 47821;

/// Events that drive the status. Notification = permission prompt / question.
const EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "Stop",
    "SessionEnd",
];

/// Marker identifying our hook commands.
fn endpoint() -> String {
    format!("127.0.0.1:{PORT}{PATH}")
}

/// Posts the hook's stdin to Escape; never fails or slows Claude Code down
/// (1 s timeout, exit 0 when Escape is not running). Works in bash and cmd.
pub fn command() -> String {
    format!(
        "curl -s -m 1 -X POST -H \"Content-Type: application/json\" --data-binary @- http://{} || exit 0",
        endpoint()
    )
}

fn is_ours(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks.iter().any(|h| {
                h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|c| c.contains(&endpoint()))
            })
        })
}

/// Whether every event has our hook.
pub fn is_installed(settings: &Value) -> bool {
    EVENTS.iter().all(|event| {
        settings
            .get("hooks")
            .and_then(|h| h.get(*event))
            .and_then(Value::as_array)
            .is_some_and(|entries| entries.iter().any(is_ours))
    })
}

/// Removes our entries (and empty event lists / `hooks` they leave behind).
pub fn uninstall(settings: &mut Value) {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    for event in EVENTS {
        if let Some(entries) = hooks.get_mut(*event).and_then(Value::as_array_mut) {
            entries.retain(|entry| !is_ours(entry));
            if entries.is_empty() {
                hooks.remove(*event);
            }
        }
    }
    if hooks.is_empty() {
        if let Some(root) = settings.as_object_mut() {
            root.remove("hooks");
        }
    }
}

/// Adds our hook to every event (idempotent). Fails if the file's shape is unexpected.
pub fn install(settings: &mut Value) -> Result<(), &'static str> {
    uninstall(settings);
    let root = settings
        .as_object_mut()
        .ok_or("settings.json должен быть объектом")?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("поле hooks в settings.json должно быть объектом")?;
    for event in EVENTS {
        let entries = hooks
            .entry(*event)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or("список хуков в settings.json имеет неожиданный вид")?;
        entries.push(json!({
            "hooks": [{ "type": "command", "command": command() }]
        }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_into_empty_settings_and_uninstalls_cleanly() {
        let mut settings = json!({});
        install(&mut settings).unwrap();
        assert!(is_installed(&settings));
        assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["type"], "command");
        uninstall(&mut settings);
        assert_eq!(settings, json!({}));
    }

    #[test]
    fn keeps_the_users_own_hooks_and_settings() {
        let mut settings = json!({
            "model": "opus",
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "notify-send done" }] }]
            },
            "permissions": { "allow": ["Bash(ls)"] }
        });
        install(&mut settings).unwrap();
        install(&mut settings).unwrap(); // idempotent
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 2);

        uninstall(&mut settings);
        assert_eq!(
            settings,
            json!({
                "model": "opus",
                "hooks": {
                    "Stop": [{ "hooks": [{ "type": "command", "command": "notify-send done" }] }]
                },
                "permissions": { "allow": ["Bash(ls)"] }
            })
        );
        // Key order is preserved.
        let keys: Vec<_> = settings.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["model", "hooks", "permissions"]);
    }

    #[test]
    fn refuses_unexpected_shapes() {
        assert!(install(&mut json!([])).is_err());
        assert!(install(&mut json!({ "hooks": [] })).is_err());
    }

    #[test]
    fn command_never_fails_claude_code() {
        let cmd = command();
        assert!(cmd.contains("-m 1"));
        assert!(cmd.ends_with("|| exit 0"));
        assert!(cmd.contains("http://127.0.0.1:47821/escape/claude-code"));
    }
}
