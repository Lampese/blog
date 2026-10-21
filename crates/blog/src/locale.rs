//! Validated message catalogs shared by templates, browser scripts, and Typst.
use anyhow::{Context, Result, bail};
use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    ops::Range,
    path::Path,
    sync::Arc,
};
use unic_langid::LanguageIdentifier;

const ENGLISH: &str = include_str!("../../../assets/i18n/en.json");
const CHINESE: &str = include_str!("../../../assets/i18n/zh-CN.json");
const CANONICAL: &str = include_str!("../resources/projection-labels-v1.json");
type Messages = BTreeMap<String, Message>;

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum Message {
    Text(String),
    Plural(BTreeMap<String, String>),
}

impl Message {
    fn templates(&self) -> Result<Vec<&str>> {
        match self {
            Self::Text(text) => Ok(vec![text]),
            Self::Plural(forms) => {
                if !forms.contains_key("other")
                    || forms.keys().any(|key| {
                        !matches!(
                            key.as_str(),
                            "zero" | "one" | "two" | "few" | "many" | "other"
                        )
                    })
                {
                    bail!("Plural messages require an 'other' form and valid CLDR categories");
                }
                Ok(forms.values().map(String::as_str).collect())
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct Locale {
    language: String,
    messages: Arc<Messages>,
    plural_languages: Arc<BTreeMap<String, String>>,
    rules: Arc<BTreeMap<String, PluralRules>>,
}

impl Locale {
    pub fn load(root: &Path, language: &str) -> Result<Self> {
        let id: LanguageIdentifier = language
            .parse()
            .context("language must be a valid language tag, such as en or zh-CN")?;
        let (mut messages, mut plural_languages) = Self::builtin(&id)?;
        let mut candidates = Vec::new();
        if id.language.as_str() == "zh" {
            candidates.push("zh-CN".to_owned());
        }
        let tag = id.to_string();
        let parts: Vec<_> = tag.split('-').collect();
        for end in 1..=parts.len() {
            candidates.push(parts[..end].join("-"));
        }
        let mut seen = BTreeSet::new();
        for candidate in candidates {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            let path = root.join("assets/i18n").join(format!("{candidate}.json"));
            match fs::read_to_string(&path) {
                Ok(source) => merge(&mut messages, &mut plural_languages, &source, &candidate)
                    .with_context(|| format!("Loading translations from {}", path.display()))?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| format!("Reading {}", path.display()));
                }
            }
        }
        Self::from_messages(id, messages, plural_languages)
    }

    /// Fixed display text underlies CRDT coordinates regardless of site language.
    pub fn canonical() -> Self {
        let id = "zh-CN".parse().expect("Valid built-in language");
        let (mut messages, mut plural_languages) =
            Self::builtin(&id).expect("Valid built-in translations");
        merge(&mut messages, &mut plural_languages, CANONICAL, "zh-CN")
            .expect("Valid versioned projection labels");
        Self::from_messages(id, messages, plural_languages).expect("Valid built-in plural rules")
    }

    fn builtin(id: &LanguageIdentifier) -> Result<(Messages, BTreeMap<String, String>)> {
        let mut messages: Messages =
            serde_json::from_str(ENGLISH).context("Invalid English catalog")?;
        for message in messages.values() {
            message.templates()?;
        }
        let mut plural_languages = messages
            .keys()
            .map(|key| (key.clone(), "en".to_owned()))
            .collect();
        if id.language.as_str() == "zh" {
            merge(&mut messages, &mut plural_languages, CHINESE, "zh-CN")?;
        }
        Ok((messages, plural_languages))
    }

    fn from_messages(
        id: LanguageIdentifier,
        messages: Messages,
        plural_languages: BTreeMap<String, String>,
    ) -> Result<Self> {
        let mut rules = BTreeMap::new();
        for language in plural_languages.values() {
            if rules.contains_key(language) {
                continue;
            }
            let primary: LanguageIdentifier = language
                .parse::<LanguageIdentifier>()?
                .language
                .as_str()
                .parse()?;
            let rule = PluralRules::create(primary, PluralRuleType::CARDINAL)
                .or_else(|_| {
                    PluralRules::create(
                        "en".parse::<LanguageIdentifier>()
                            .expect("Valid built-in language"),
                        PluralRuleType::CARDINAL,
                    )
                })
                .map_err(|error| anyhow::anyhow!("Cannot initialize plural rules: {error}"))?;
            rules.insert(language.clone(), rule);
        }
        Ok(Self {
            language: id.to_string(),
            messages: Arc::new(messages),
            plural_languages: Arc::new(plural_languages),
            rules: Arc::new(rules),
        })
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn text(&self, key: &str) -> Result<String> {
        self.format(key, &BTreeMap::new())
    }

    pub fn format(&self, key: &str, arguments: &BTreeMap<String, String>) -> Result<String> {
        let message = self
            .messages
            .get(key)
            .with_context(|| format!("Unknown translation key: {key}"))?;
        let template = match message {
            Message::Text(text) => text,
            Message::Plural(forms) => {
                let count = arguments
                    .get("count")
                    .context("Plural messages require a count argument")?;
                let category = self.rules[&self.plural_languages[key]]
                    .select(count.as_str())
                    .map_err(|error| anyhow::anyhow!("Invalid plural count: {error}"))?;
                let category = match category {
                    PluralCategory::ZERO => "zero",
                    PluralCategory::ONE => "one",
                    PluralCategory::TWO => "two",
                    PluralCategory::FEW => "few",
                    PluralCategory::MANY => "many",
                    PluralCategory::OTHER => "other",
                };
                forms.get(category).unwrap_or(&forms["other"])
            }
        };
        let mut rendered = String::new();
        let mut end = 0;
        for range in placeholders(template)? {
            let name = &template[range.start + 1..range.end - 1];
            rendered.push_str(&template[end..range.start]);
            rendered.push_str(
                arguments.get(name).with_context(|| {
                    format!("Missing argument '{name}' for translation '{key}'")
                })?,
            );
            end = range.end;
        }
        rendered.push_str(&template[end..]);
        Ok(rendered)
    }

    pub fn messages_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self.messages.as_ref())?)
    }

    pub fn script_data(&self) -> Result<String> {
        Ok(serde_json::to_string(
            &serde_json::json!({"language":self.language,"messages":self.messages.as_ref(),"pluralLanguages":self.plural_languages.as_ref()}),
        )?
        .replace('<', "\\u003c"))
    }
}

fn merge(
    messages: &mut Messages,
    plural_languages: &mut BTreeMap<String, String>,
    source: &str,
    language: &str,
) -> Result<()> {
    let incoming: Messages = serde_json::from_str(source).context("Invalid translation catalog")?;
    for (key, message) in incoming {
        let original = messages
            .get(&key)
            .with_context(|| format!("Unknown translation key: {key}"))?;
        let expected = argument_names(original.templates()?[0])?;
        for template in message.templates()? {
            if argument_names(template)? != expected {
                bail!("Translation '{key}' must preserve placeholders {expected:?}");
            }
        }
        if matches!(message, Message::Plural(_)) && !expected.contains("count") {
            bail!("Plural translation '{key}' requires a count placeholder");
        }
        plural_languages.insert(key.clone(), language.to_owned());
        messages.insert(key, message);
    }
    Ok(())
}

fn argument_names(template: &str) -> Result<BTreeSet<&str>> {
    Ok(placeholders(template)?
        .into_iter()
        .map(|range| &template[range.start + 1..range.end - 1])
        .collect())
}

fn placeholders(template: &str) -> Result<Vec<Range<usize>>> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    while let Some(start) = template[offset..].find('{') {
        let start = offset + start;
        let end = start
            + template[start..]
                .find('}')
                .context("Unclosed translation placeholder")?
            + 1;
        let name = &template[start + 1..end - 1];
        if name.is_empty()
            || !name
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
        {
            bail!("Invalid translation placeholder: {name}");
        }
        ranges.push(start..end);
        offset = end;
    }
    Ok(ranges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(locale: &Locale, key: &str, count: usize) -> Result<String> {
        locale.format(
            key,
            &BTreeMap::from([("count".to_owned(), count.to_string())]),
        )
    }

    #[test]
    fn bundled_catalogs_have_matching_keys_and_placeholders() -> Result<()> {
        let english: Messages = serde_json::from_str(ENGLISH)?;
        let chinese: Messages = serde_json::from_str(CHINESE)?;
        assert_eq!(
            english.keys().collect::<Vec<_>>(),
            chinese.keys().collect::<Vec<_>>()
        );
        let mut merged = english;
        let mut origins = BTreeMap::new();
        merge(&mut merged, &mut origins, CHINESE, "zh-CN")?;
        Ok(())
    }

    #[test]
    fn region_overrides_fall_back_to_their_parent_catalog() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir_all(root.path().join("assets/i18n"))?;
        fs::write(
            root.path().join("assets/i18n/en.json"),
            r#"{"search":"Find"}"#,
        )?;
        fs::write(
            root.path().join("assets/i18n/en-GB.json"),
            r#"{"search":"Find writing"}"#,
        )?;
        let locale = Locale::load(root.path(), "en-gb")?;
        assert_eq!(locale.language(), "en-GB");
        assert_eq!(locale.text("search")?, "Find writing");
        assert_eq!(locale.text("articles")?, "Articles");
        assert_eq!(count(&locale, "article_count", 1)?, "1 article");
        assert_eq!(count(&locale, "article_count", 2)?, "2 articles");
        Ok(())
    }

    #[test]
    fn fallback_messages_keep_their_own_plural_rules() -> Result<()> {
        let root = tempfile::tempdir()?;
        let fallback = Locale::load(root.path(), "fr")?;
        assert_eq!(count(&fallback, "article_count", 0)?, "0 articles");
        fs::create_dir_all(root.path().join("assets/i18n"))?;
        fs::write(
            root.path().join("assets/i18n/fr.json"),
            r#"{"article_count":{"one":"singular {count}","other":"plural {count}"}}"#,
        )?;
        let translated = Locale::load(root.path(), "fr")?;
        assert_eq!(count(&translated, "article_count", 0)?, "singular 0");
        assert_eq!(count(&translated, "article_count", 2)?, "plural 2");
        assert_eq!(count(&translated, "search_results", 0)?, "Found 0 articles");
        Ok(())
    }

    #[test]
    fn invalid_catalogs_and_unsafe_language_paths_are_rejected() -> Result<()> {
        let root = tempfile::tempdir()?;
        assert!(Locale::load(root.path(), "../../outside").is_err());
        fs::create_dir_all(root.path().join("assets/i18n"))?;
        for invalid in [
            r#"{"unknown_key":"text"}"#,
            r#"{"article_count":{"other":"missing count"}}"#,
            r#"{"search":"{unclosed"}"#,
            r#"{"search":{"other":"plural without count"}}"#,
        ] {
            fs::write(root.path().join("assets/i18n/en.json"), invalid)?;
            assert!(Locale::load(root.path(), "en").is_err(), "{invalid}");
        }
        Ok(())
    }

    #[test]
    fn interpolation_is_single_pass_and_script_data_is_escaped() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir_all(root.path().join("assets/i18n"))?;
        fs::write(
            root.path().join("assets/i18n/en.json"),
            r#"{"search":"</script><img src=x>"}"#,
        )?;
        let locale = Locale::load(root.path(), "en")?;
        let arguments = BTreeMap::from([("title".to_owned(), "{title}</script>".to_owned())]);
        assert_eq!(
            locale.format("source_title", &arguments)?,
            "Source: {title}</script>"
        );
        assert!(!locale.script_data()?.contains("</script>"));
        let parsed: serde_json::Value = serde_json::from_str(&locale.script_data()?)?;
        assert_eq!(parsed["messages"]["search"], "</script><img src=x>");
        Ok(())
    }
}
