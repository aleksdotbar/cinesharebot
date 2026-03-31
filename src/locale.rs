#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    En,
    Ru,
}

impl Locale {
    pub fn from_language_code(language_code: Option<&str>) -> Self {
        let Some(language_code) = language_code.map(str::trim).filter(|value| !value.is_empty()) else {
            return Self::En;
        };

        let normalized = language_code.replace('_', "-").to_ascii_lowercase();
        if normalized == "ru" || normalized.starts_with("ru-") {
            Self::Ru
        } else {
            Self::En
        }
    }

    pub fn from_query(query: &str) -> Option<Self> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return None;
        }

        if trimmed.chars().any(is_cyrillic) {
            Some(Self::Ru)
        } else {
            Some(Self::En)
        }
    }

    pub fn tmdb_language(self) -> Option<&'static str> {
        match self {
            Self::En => None,
            Self::Ru => Some("ru-RU"),
        }
    }
}

fn is_cyrillic(character: char) -> bool {
    matches!(character as u32, 0x0400..=0x04FF | 0x0500..=0x052F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F | 0x1C80..=0x1C8F)
}

#[cfg(test)]
mod tests {
    use super::Locale;

    #[test]
    fn parses_russian_language_codes() {
        assert_eq!(Locale::from_language_code(Some("ru")), Locale::Ru);
        assert_eq!(Locale::from_language_code(Some("ru-RU")), Locale::Ru);
        assert_eq!(Locale::from_language_code(Some("ru_RU")), Locale::Ru);
    }

    #[test]
    fn falls_back_to_english_for_missing_or_unknown_language_codes() {
        assert_eq!(Locale::from_language_code(None), Locale::En);
        assert_eq!(Locale::from_language_code(Some("")), Locale::En);
        assert_eq!(Locale::from_language_code(Some("en")), Locale::En);
        assert_eq!(Locale::from_language_code(Some("sr")), Locale::En);
    }

    #[test]
    fn detects_query_locale_from_script() {
        assert_eq!(Locale::from_query("джон уик"), Some(Locale::Ru));
        assert_eq!(Locale::from_query("john wick"), Some(Locale::En));
        assert_eq!(Locale::from_query("   "), None);
    }
}
