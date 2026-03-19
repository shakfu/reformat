//! Case format definitions and conversion logic

/// Supported case formats for identifier conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseFormat {
    /// camelCase: firstName, lastName
    CamelCase,
    /// PascalCase: FirstName, LastName
    PascalCase,
    /// snake_case: first_name, last_name
    SnakeCase,
    /// SCREAMING_SNAKE_CASE: FIRST_NAME, LAST_NAME
    ScreamingSnakeCase,
    /// kebab-case: first-name, last-name
    KebabCase,
    /// SCREAMING-KEBAB-CASE: FIRST-NAME, LAST-NAME
    ScreamingKebabCase,
}

impl CaseFormat {
    /// Returns the regex pattern for identifying this case format
    pub fn pattern(&self) -> &str {
        match self {
            CaseFormat::CamelCase => r"\b[a-z]+(?:[A-Z][a-z0-9]*)+\b",
            CaseFormat::PascalCase => r"\b[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]*)+\b",
            CaseFormat::SnakeCase => r"\b[a-z]+(?:_[a-z0-9]+)+\b",
            CaseFormat::ScreamingSnakeCase => r"\b[A-Z]+(?:_[A-Z0-9]+)+\b",
            CaseFormat::KebabCase => r"\b[a-z]+(?:-[a-z0-9]+)+\b",
            CaseFormat::ScreamingKebabCase => r"\b[A-Z]+(?:-[A-Z0-9]+)+\b",
        }
    }

    /// Splits a string into words based on this case format
    pub fn split_words(&self, text: &str) -> Vec<String> {
        match self {
            CaseFormat::CamelCase | CaseFormat::PascalCase => {
                // Split on uppercase letters manually since regex doesn't support lookahead
                let mut words = Vec::new();
                let mut current_word = String::new();

                for ch in text.chars() {
                    if ch.is_uppercase() && !current_word.is_empty() {
                        words.push(current_word.to_lowercase());
                        current_word = String::new();
                    }
                    current_word.push(ch);
                }

                if !current_word.is_empty() {
                    words.push(current_word.to_lowercase());
                }

                words
            }
            CaseFormat::SnakeCase | CaseFormat::ScreamingSnakeCase => {
                // Split on underscores
                text.split('_')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_lowercase())
                    .collect()
            }
            CaseFormat::KebabCase | CaseFormat::ScreamingKebabCase => {
                // Split on hyphens
                text.split('-')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_lowercase())
                    .collect()
            }
        }
    }

    /// Joins words into this case format with optional prefix and suffix
    pub fn join_words(&self, words: &[String], prefix: &str, suffix: &str) -> String {
        if words.is_empty() {
            return String::new();
        }

        let result = match self {
            CaseFormat::CamelCase => {
                let first = words[0].to_lowercase();
                let rest: String = words[1..]
                    .iter()
                    .map(|w| {
                        let mut chars = w.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => first.to_uppercase().chain(chars).collect(),
                        }
                    })
                    .collect();
                format!("{}{}", first, rest)
            }
            CaseFormat::PascalCase => words
                .iter()
                .map(|w| {
                    let mut chars = w.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().chain(chars).collect(),
                    }
                })
                .collect::<String>(),
            CaseFormat::SnakeCase => words
                .iter()
                .map(|w| w.to_lowercase())
                .collect::<Vec<_>>()
                .join("_"),
            CaseFormat::ScreamingSnakeCase => words
                .iter()
                .map(|w| w.to_uppercase())
                .collect::<Vec<_>>()
                .join("_"),
            CaseFormat::KebabCase => words
                .iter()
                .map(|w| w.to_lowercase())
                .collect::<Vec<_>>()
                .join("-"),
            CaseFormat::ScreamingKebabCase => words
                .iter()
                .map(|w| w.to_uppercase())
                .collect::<Vec<_>>()
                .join("-"),
        };

        format!("{}{}{}", prefix, result, suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camel_split() {
        let words = CaseFormat::CamelCase.split_words("firstName");
        assert_eq!(words, vec!["first", "name"]);
    }

    #[test]
    fn test_snake_split() {
        let words = CaseFormat::SnakeCase.split_words("first_name");
        assert_eq!(words, vec!["first", "name"]);
    }

    #[test]
    fn test_camel_join() {
        let words = vec!["first".to_string(), "name".to_string()];
        assert_eq!(
            CaseFormat::CamelCase.join_words(&words, "", ""),
            "firstName"
        );
    }

    #[test]
    fn test_snake_join() {
        let words = vec!["first".to_string(), "name".to_string()];
        assert_eq!(
            CaseFormat::SnakeCase.join_words(&words, "", ""),
            "first_name"
        );
    }

    #[test]
    fn test_with_prefix_suffix() {
        let words = vec!["first".to_string(), "name".to_string()];
        assert_eq!(
            CaseFormat::SnakeCase.join_words(&words, "old_", "_v1"),
            "old_first_name_v1"
        );
    }
}
