//! One-time move of the app's folders after the rename Escape → Cuba
//! (identifier `com.escape.app` → `com.cuba.app`). Runs before Tauri creates
//! any folder, so the new ones do not exist yet on the first start.

use std::path::{Path, PathBuf};

const OLD: &str = "com.escape.app";
const NEW: &str = "com.cuba.app";

/// What happened to one folder.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Moved,
    /// Nothing to move, or the new folder is already there.
    Skipped,
    Failed(String),
}

/// Moves `%APPDATA%` and `%LOCALAPPDATA%` folders to the new name. Returns lines
/// for the log, which is not running yet at this point.
pub fn move_old_data() -> Vec<String> {
    let mut lines = Vec::new();
    for var in ["APPDATA", "LOCALAPPDATA"] {
        let Some(base) = std::env::var_os(var).map(PathBuf::from) else {
            continue;
        };
        let (old, new) = (base.join(OLD), base.join(NEW));
        match move_dir(&old, &new) {
            Outcome::Moved => lines.push(format!("moved {} to {}", old.display(), new.display())),
            Outcome::Skipped => {}
            Outcome::Failed(err) => lines.push(format!(
                "cannot move {} to {}: {err}",
                old.display(),
                new.display()
            )),
        }
    }
    lines
}

/// Renames `old` to `new` when only `old` exists (both are on the same drive).
pub fn move_dir(old: &Path, new: &Path) -> Outcome {
    if !old.is_dir() || new.exists() {
        return Outcome::Skipped;
    }
    match std::fs::rename(old, new) {
        Ok(()) => Outcome::Moved,
        Err(err) => Outcome::Failed(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cuba-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn moves_the_old_folder_with_its_files() {
        let base = temp("move");
        let (old, new) = (base.join(OLD), base.join(NEW));
        std::fs::create_dir_all(old.join("clips")).unwrap();
        std::fs::write(old.join("settings.json"), "{}").unwrap();
        assert_eq!(move_dir(&old, &new), Outcome::Moved);
        assert!(!old.exists());
        assert_eq!(
            std::fs::read_to_string(new.join("settings.json")).unwrap(),
            "{}"
        );
        assert!(new.join("clips").is_dir());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn leaves_everything_when_the_new_folder_exists_or_the_old_does_not() {
        let base = temp("skip");
        let (old, new) = (base.join(OLD), base.join(NEW));
        assert_eq!(move_dir(&old, &new), Outcome::Skipped);
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        assert_eq!(move_dir(&old, &new), Outcome::Skipped);
        assert!(old.exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
