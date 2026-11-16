//! What the island does with notifications, independent of Windows.

use std::collections::HashSet;

use serde::Serialize;

/// One notification from the Notification Center. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// Windows' id, used to open or remove it.
    pub id: u32,
    /// App display name ("Telegram").
    pub app: String,
    pub title: String,
    pub body: String,
    /// Unix ms.
    pub time: i64,
    /// App logo as a data URL, if Windows gave one.
    pub icon: Option<String>,
    /// Picture of the sender or chat as a data URL, if the notification carried one.
    pub avatar: Option<String>,
    /// The app's AppUserModelID (to open it or change its banner setting).
    #[serde(skip)]
    pub app_id: String,
}

/// Finds notifications that arrived since the previous look.
#[derive(Default)]
pub struct Tracker {
    seen: HashSet<u32>,
    primed: bool,
}

impl Tracker {
    /// The first look only remembers what is there: old notifications are not news.
    pub fn arrived(&mut self, current: &[Item]) -> Vec<Item> {
        let ids: HashSet<u32> = current.iter().map(|item| item.id).collect();
        let fresh = if self.primed {
            current
                .iter()
                .filter(|item| !self.seen.contains(&item.id))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        self.seen = ids;
        self.primed = true;
        fresh
    }
}

/// A toast's text lines → (title, body): the first non-empty line, then the rest.
pub fn split_texts(lines: &[String]) -> (String, String) {
    let mut texts = lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty());
    let title = texts.next().unwrap_or_default().to_owned();
    let body = texts.collect::<Vec<_>>().join("\n");
    (title, body)
}

/// Whether the user wants to see this app's notifications.
pub fn allowed(app: &str, muted: &[String]) -> bool {
    !muted.iter().any(|m| m.eq_ignore_ascii_case(app))
}

/// Newest first, at most `limit`, without muted apps.
pub fn visible(items: &[Item], muted: &[String], limit: usize) -> Vec<Item> {
    let mut list: Vec<Item> = items
        .iter()
        .filter(|item| allowed(&item.app, muted))
        .cloned()
        .collect();
    list.sort_by_key(|item| std::cmp::Reverse(item.time));
    list.truncate(limit);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u32, app: &str, time: i64) -> Item {
        Item {
            id,
            app: app.into(),
            title: "t".into(),
            body: String::new(),
            time,
            icon: None,
            avatar: None,
            app_id: String::new(),
        }
    }

    #[test]
    fn reports_only_what_arrived_after_the_first_look() {
        let mut tracker = Tracker::default();
        assert!(tracker
            .arrived(&[item(1, "A", 0), item(2, "B", 0)])
            .is_empty());
        let fresh = tracker.arrived(&[item(2, "B", 0), item(3, "C", 5)]);
        assert_eq!(fresh, [item(3, "C", 5)]);
        // A removed and re-posted id counts as new again.
        assert!(tracker.arrived(&[item(3, "C", 5)]).is_empty());
        assert_eq!(tracker.arrived(&[item(2, "B", 9)]), [item(2, "B", 9)]);
    }

    #[test]
    fn splits_title_and_body() {
        let lines = ["  ", "Маша", "Привет!", "Ты где?"].map(String::from);
        assert_eq!(
            split_texts(&lines),
            ("Маша".into(), "Привет!\nТы где?".into())
        );
        assert_eq!(split_texts(&[]), (String::new(), String::new()));
    }

    #[test]
    fn hides_muted_apps_and_sorts_newest_first() {
        let items = [
            item(1, "Discord", 10),
            item(2, "Telegram", 30),
            item(3, "OneDrive", 20),
        ];
        let muted = vec!["discord".to_owned()];
        let list = visible(&items, &muted, 5);
        assert_eq!(list.iter().map(|i| i.id).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(visible(&items, &[], 1).len(), 1);
    }
}
