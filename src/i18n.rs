//! Runtime language selection for user-facing messages.

use std::cell::Cell;

thread_local! {
    static LANGUAGE_OVERRIDE: Cell<Option<Language>> = const { Cell::new(None) };
}

/// Supported output languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Korean,
    English,
}

/// Returns the runtime language.
///
/// Priority:
/// 1) `TREX_LANG` override (`ko`/`en`)
/// 2) `LC_ALL`
/// 3) `LC_MESSAGES`
/// 4) `LANG`
/// 5) English fallback
pub fn current_language() -> Language {
    if let Some(language) = LANGUAGE_OVERRIDE.with(Cell::get) {
        return language;
    }

    if let Ok(value) = std::env::var("TREX_LANG") {
        if let Some(language) = parse_language_hint(&value) {
            return language;
        }
    }

    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            if value.trim().is_empty() {
                continue;
            }
            if let Some(language) = parse_language_hint(&value) {
                return language;
            }
        }
    }

    Language::English
}

pub fn is_korean() -> bool {
    current_language() == Language::Korean
}

pub fn text<'a>(ko: &'a str, en: &'a str) -> &'a str {
    if is_korean() { ko } else { en }
}

/// Runs a closure with a thread-local language override.
///
/// Useful for request-scoped language selection in HTTP APIs.
pub fn with_language<T>(language: Language, f: impl FnOnce() -> T) -> T {
    struct ResetGuard(Option<Language>);

    impl Drop for ResetGuard {
        fn drop(&mut self) {
            LANGUAGE_OVERRIDE.with(|slot| slot.set(self.0));
        }
    }

    LANGUAGE_OVERRIDE.with(|slot| {
        let previous = slot.replace(Some(language));
        let _guard = ResetGuard(previous);
        f()
    })
}

/// Parses `Accept-Language` header and returns supported language preference.
///
/// Currently supports Korean (`ko*`) and English (`en*`, `C`, `POSIX`).
/// If multiple supported values exist, the one with the highest `q` is chosen.
pub fn language_from_accept_language(raw: &str) -> Option<Language> {
    let mut best: Option<(Language, f32, usize)> = None;

    for (index, part) in raw.split(',').enumerate() {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }

        let mut segments = token.split(';');
        let language_token = segments.next().unwrap_or_default().trim();
        let Some(language) = parse_language_hint(language_token) else {
            continue;
        };

        let mut quality = 1.0f32;
        for segment in segments {
            let segment = segment.trim();
            if let Some(value) = segment.strip_prefix("q=") {
                if let Ok(parsed) = value.parse::<f32>() {
                    quality = parsed.clamp(0.0, 1.0);
                }
            }
        }

        match best {
            None => best = Some((language, quality, index)),
            Some((_, best_quality, best_index)) => {
                if quality > best_quality || (quality == best_quality && index < best_index) {
                    best = Some((language, quality, index));
                }
            }
        }
    }

    best.map(|(language, _, _)| language)
}

fn parse_language_hint(raw: &str) -> Option<Language> {
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    if normalized == "ko"
        || normalized.starts_with("ko_")
        || normalized.starts_with("ko-")
        || normalized.starts_with("korean")
    {
        return Some(Language::Korean);
    }

    if normalized == "en"
        || normalized.starts_with("en_")
        || normalized.starts_with("en-")
        || normalized.starts_with("english")
        || normalized == "c"
        || normalized == "posix"
    {
        return Some(Language::English);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{Language, language_from_accept_language, parse_language_hint};

    #[test]
    fn parse_korean_variants() {
        assert_eq!(parse_language_hint("ko"), Some(Language::Korean));
        assert_eq!(parse_language_hint("ko_KR.UTF-8"), Some(Language::Korean));
        assert_eq!(parse_language_hint("Korean"), Some(Language::Korean));
    }

    #[test]
    fn parse_english_variants() {
        assert_eq!(parse_language_hint("en"), Some(Language::English));
        assert_eq!(parse_language_hint("en_US.UTF-8"), Some(Language::English));
        assert_eq!(parse_language_hint("C"), Some(Language::English));
    }

    #[test]
    fn parse_accept_language_header() {
        assert_eq!(
            language_from_accept_language("en-US,en;q=0.9,ko;q=0.8"),
            Some(Language::English)
        );
        assert_eq!(
            language_from_accept_language("fr-FR,ko-KR;q=0.9,en;q=0.7"),
            Some(Language::Korean)
        );
    }
}
