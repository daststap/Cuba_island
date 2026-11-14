//! Fuzzy ranking of launcher entries (PLAN.md stage 4).

use std::collections::HashMap;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Anything that can be found by name. `keywords` are extra searchable words.
pub struct Candidate<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub keywords: &'a str,
}

/// Bonus when the query is the start of a word in the name ("chr" → "Google Chrome").
const WORD_START_BONUS: u32 = 1000;
/// Bonus per doubling of launches ("recently used ranks higher").
const LAUNCH_BONUS: f64 = 120.0;
/// Without exact matches, fuzzy matches scoring below 60% of the best are dropped.
const FUZZY_KEEP_TENTHS: u32 = 6;

fn word_starts_with(name: &str, query: &str) -> bool {
    name.split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(query))
}

/// Returns candidate ids, best first, at most `limit`.
pub fn rank<'a>(
    query: &str,
    candidates: impl IntoIterator<Item = Candidate<'a>>,
    launches: &HashMap<String, u32>,
    limit: usize,
) -> Vec<&'a str> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let lower_query = query.to_lowercase();
    let mut buf = Vec::new();

    struct Scored<'a> {
        total: u32,
        fuzzy: u32,
        /// The query appears as-is (substring) — not just as scattered letters.
        strong: bool,
        name: &'a str,
        id: &'a str,
    }
    let all: Vec<Scored<'a>> = candidates
        .into_iter()
        .filter_map(|c| {
            let haystack = if c.keywords.is_empty() {
                c.name.to_owned()
            } else {
                format!("{} {}", c.name, c.keywords)
            };
            let fuzzy = pattern.score(Utf32Str::new(&haystack, &mut buf), &mut matcher)?;
            let lower = haystack.to_lowercase();
            let word_start = word_starts_with(&lower, &lower_query);
            let count = launches.get(c.id).copied().unwrap_or(0);
            // log2(1 + launches) keeps a frequent app ahead without burying better matches.
            let launch_bonus =
                (f64::from(count).ln_1p() / std::f64::consts::LN_2 * LAUNCH_BONUS) as u32;
            Some(Scored {
                total: fuzzy + if word_start { WORD_START_BONUS } else { 0 } + launch_bonus,
                fuzzy,
                strong: word_start || lower.contains(&lower_query),
                name: c.name,
                id: c.id,
            })
        })
        .collect();

    // Scattered-letter matches ("tele" → "TeamSpeak") are noise when real matches
    // exist; with only fuzzy matches (a typo), keep those close to the best one.
    let has_strong = all.iter().any(|s| s.strong);
    let best_fuzzy = all.iter().map(|s| s.fuzzy).max().unwrap_or(0);
    let mut scored: Vec<(u32, &'a str, &'a str)> = all
        .into_iter()
        .filter(|s| {
            if has_strong {
                s.strong
            } else {
                s.fuzzy * 10 >= best_fuzzy * FUZZY_KEEP_TENTHS
            }
        })
        .map(|s| (s.total, s.name, s.id))
        .collect();

    // Higher score first; shorter names win ties ("Chrome" over "Chrome Canary").
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, id)| id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPS: &[(&str, &str)] = &[
        ("chrome", "Google Chrome"),
        ("canary", "Google Chrome Canary"),
        ("calc", "Калькулятор"),
        ("settings", "Параметры"),
        ("telegram", "Telegram Desktop"),
        ("charmap", "Таблица символов"),
        ("teamspeak", "TeamSpeak 3 Client"),
        ("headliners", "The Headliners"),
    ];

    fn search(query: &str, launches: &[(&str, u32)]) -> Vec<&'static str> {
        let launches = launches
            .iter()
            .map(|(id, n)| ((*id).to_owned(), *n))
            .collect();
        rank(
            query,
            APPS.iter().map(|(id, name)| Candidate {
                id,
                name,
                keywords: "",
            }),
            &launches,
            8,
        )
    }

    #[test]
    fn finds_by_word_start_and_prefers_shorter_names() {
        assert_eq!(search("chr", &[])[..2], ["chrome", "canary"]);
        assert_eq!(search("tele", &[])[0], "telegram");
    }

    #[test]
    fn drops_scattered_letter_matches_when_real_ones_exist() {
        assert_eq!(search("tele", &[]), ["telegram"]);
    }

    #[test]
    fn keeps_close_fuzzy_matches_for_typos() {
        assert_eq!(search("telgrm", &[])[0], "telegram");
    }

    #[test]
    fn matches_cyrillic_case_insensitively() {
        assert_eq!(search("кальк", &[])[0], "calc");
        assert_eq!(search("ПАРАМ", &[])[0], "settings");
    }

    #[test]
    fn frequently_launched_apps_rank_higher() {
        assert_eq!(search("chr", &[("canary", 20)])[0], "canary");
    }

    #[test]
    fn searches_keywords_and_respects_limits() {
        let found = rank(
            "lock",
            [Candidate {
                id: "cmd:lock",
                name: "Заблокировать компьютер",
                keywords: "lock",
            }],
            &HashMap::new(),
            8,
        );
        assert_eq!(found, ["cmd:lock"]);
        assert!(search("", &[]).is_empty());
        assert!(search("zzzzqqq", &[]).is_empty());
    }
}
