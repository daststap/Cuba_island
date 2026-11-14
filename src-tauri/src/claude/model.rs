//! Claude Code sessions as seen through its hooks (PLAN.md stage 6).

use serde::{Deserialize, Serialize};

/// The fields we use from a hook's stdin JSON; everything else is ignored.
#[derive(Debug, Deserialize)]
pub struct HookEvent {
    pub session_id: String,
    pub hook_event_name: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Thinking or running tools.
    Working,
    /// Asking for permission or input.
    Waiting,
    /// Finished its turn.
    Idle,
}

/// One session for the UI. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    /// Folder name of the project.
    pub project: String,
    pub state: SessionState,
    /// Tool in use while working (Edit, Bash…).
    pub tool: Option<String>,
    /// What Claude is waiting for.
    pub message: Option<String>,
    pub updated_at: i64,
}

/// A working session silent for this long probably crashed or was closed.
const STALE_WORKING_MS: i64 = 10 * 60 * 1000;
/// Idle sessions are forgotten after this long.
const FORGET_IDLE_MS: i64 = 60 * 60 * 1000;

#[derive(Debug, Default)]
pub struct Sessions {
    list: Vec<Session>,
}

fn project_name(cwd: &str) -> String {
    cwd.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("Claude Code")
        .to_owned()
}

impl Sessions {
    pub fn list(&self) -> &[Session] {
        &self.list
    }

    /// Applies a hook event. Returns `true` if a session just started waiting for the user.
    pub fn apply(&mut self, event: HookEvent, now: i64) -> bool {
        if event.hook_event_name == "SessionEnd" {
            self.list.retain(|s| s.id != event.session_id);
            return false;
        }
        let index = match self.list.iter().position(|s| s.id == event.session_id) {
            Some(index) => index,
            None => {
                self.list.push(Session {
                    id: event.session_id.clone(),
                    project: project_name(&event.cwd),
                    state: SessionState::Idle,
                    tool: None,
                    message: None,
                    updated_at: now,
                });
                self.list.len() - 1
            }
        };
        let session = &mut self.list[index];
        let was_waiting = session.state == SessionState::Waiting;
        session.updated_at = now;
        if !event.cwd.is_empty() {
            session.project = project_name(&event.cwd);
        }
        match event.hook_event_name.as_str() {
            "UserPromptSubmit" => {
                session.state = SessionState::Working;
                session.tool = None;
                session.message = None;
            }
            "PreToolUse" => {
                session.state = SessionState::Working;
                session.tool = event.tool_name;
                session.message = None;
            }
            "PostToolUse" => {
                // Permission was granted (or not needed) and the tool ran.
                session.state = SessionState::Working;
                session.message = None;
            }
            "Notification" => {
                session.state = SessionState::Waiting;
                session.message = event.message;
            }
            "Stop" => {
                session.state = SessionState::Idle;
                session.tool = None;
                session.message = None;
            }
            // SessionStart, SubagentStop and others only refresh the session.
            _ => {}
        }
        !was_waiting && session.state == SessionState::Waiting
    }

    /// Ages out stale state. Returns whether anything changed.
    pub fn expire(&mut self, now: i64) -> bool {
        let before = self.list.clone();
        for session in &mut self.list {
            if session.state == SessionState::Working && now - session.updated_at > STALE_WORKING_MS
            {
                session.state = SessionState::Idle;
                session.tool = None;
            }
        }
        self.list
            .retain(|s| s.state != SessionState::Idle || now - s.updated_at <= FORGET_IDLE_MS);
        self.list != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str) -> HookEvent {
        HookEvent {
            session_id: "s1".into(),
            hook_event_name: name.into(),
            cwd: "C:\\code\\escape".into(),
            tool_name: None,
            message: None,
        }
    }

    #[test]
    fn follows_a_turn() {
        let mut sessions = Sessions::default();
        sessions.apply(event("UserPromptSubmit"), 1);
        assert_eq!(sessions.list()[0].state, SessionState::Working);
        assert_eq!(sessions.list()[0].project, "escape");

        let mut tool = event("PreToolUse");
        tool.tool_name = Some("Edit".into());
        sessions.apply(tool, 2);
        assert_eq!(sessions.list()[0].tool.as_deref(), Some("Edit"));

        let mut ask = event("Notification");
        ask.message = Some("Claude needs your permission to use Bash".into());
        assert!(
            sessions.apply(ask, 3),
            "starting to wait should be reported"
        );
        assert_eq!(sessions.list()[0].state, SessionState::Waiting);

        assert!(!sessions.apply(event("PostToolUse"), 4));
        assert_eq!(sessions.list()[0].state, SessionState::Working);

        sessions.apply(event("Stop"), 5);
        assert_eq!(sessions.list()[0].state, SessionState::Idle);

        sessions.apply(event("SessionEnd"), 6);
        assert!(sessions.list().is_empty());
    }

    #[test]
    fn repeated_notifications_report_once() {
        let mut sessions = Sessions::default();
        assert!(sessions.apply(event("Notification"), 1));
        assert!(!sessions.apply(event("Notification"), 2));
    }

    #[test]
    fn tracks_sessions_separately() {
        let mut sessions = Sessions::default();
        sessions.apply(event("UserPromptSubmit"), 1);
        let mut other = event("UserPromptSubmit");
        other.session_id = "s2".into();
        other.cwd = "/home/me/api/".into();
        sessions.apply(other, 1);
        assert_eq!(sessions.list().len(), 2);
        assert_eq!(sessions.list()[1].project, "api");
    }

    #[test]
    fn expires_stale_and_old_sessions() {
        let mut sessions = Sessions::default();
        sessions.apply(event("UserPromptSubmit"), 0);
        assert!(!sessions.expire(STALE_WORKING_MS));
        assert!(sessions.expire(STALE_WORKING_MS + 1));
        assert_eq!(sessions.list()[0].state, SessionState::Idle);
        assert!(sessions.expire(FORGET_IDLE_MS + 1));
        assert!(sessions.list().is_empty());
    }

    #[test]
    fn parses_real_hook_payloads() {
        let json = r#"{"session_id":"abc","transcript_path":"x","cwd":"D:\\Island",
            "hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"ls"}}"#;
        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.tool_name.as_deref(), Some("Bash"));
        assert_eq!(project_name(&event.cwd), "Island");
    }
}
