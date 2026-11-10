//! Format-neutral lexical records. Importers supply source identity and keep
//! original metadata instead of silently dropping unsupported source fields.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Dict,
    Wiktextract,
    Stardict,
    Tei,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DictionarySource {
    /// Stable identifier within the local Lexiva catalog, not a display name.
    pub id: String,
    pub title: String,
    pub format: SourceFormat,
    /// Read-only source location; no importer may rewrite the original.
    pub origin: Option<String>,
    pub headword_language: Option<String>,
    pub definition_language: Option<String>,
    pub license: Option<String>,
    pub attribution: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LexicalEntry {
    pub source_id: String,
    /// Stable identity within a dictionary, even when two entries share a lemma.
    pub source_entry_id: String,
    pub lemma: String,
    pub headword_language: Option<String>,
    pub definition_language: Option<String>,
    pub part_of_speech: Option<String>,
    #[serde(default)]
    pub senses: Vec<Sense>,
    #[serde(default)]
    pub pronunciations: Vec<Pronunciation>,
    #[serde(default)]
    pub etymologies: Vec<String>,
    #[serde(default)]
    pub forms: Vec<WordForm>,
    #[serde(default)]
    pub relations: Vec<RelatedTerm>,
    /// Lossless adapter-specific fields for later reconstruction/audit.
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sense {
    pub number: Option<String>,
    /// The language of this specific sense may override the entry default.
    pub definition_language: Option<String>,
    #[serde(default)]
    pub glosses: Vec<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub examples: Vec<Example>,
    #[serde(default)]
    pub children: Vec<Sense>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Example {
    pub text: String,
    pub translation: Option<String>,
    pub reference: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pronunciation {
    pub ipa: Option<String>,
    pub audio: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WordForm {
    pub spelling: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelatedTerm {
    pub kind: String,
    pub term: String,
    pub language: Option<String>,
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && id == id.trim()
}

/// Accepts a conservative BCP-47-shaped code. Unknown language is None, never
/// silently inferred from the language used for a dictionary's filename.
pub fn valid_language_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 64
        && tag.split('-').all(|part| {
            (1..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn check_language(lang: Option<&str>) -> Result<(), &'static str> {
    if lang.is_some_and(|code| !valid_language_tag(code)) {
        return Err("invalid language tag");
    }
    Ok(())
}

impl DictionarySource {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !valid_id(&self.id) {
            return Err("invalid dictionary source ID");
        }
        if self.title.trim().is_empty() || self.title.len() > 1024 {
            return Err("invalid dictionary title");
        }
        check_language(self.headword_language.as_deref())?;
        check_language(self.definition_language.as_deref())?;
        Ok(())
    }
}

impl LexicalEntry {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !valid_id(&self.source_id) || !valid_id(&self.source_entry_id) {
            return Err("invalid entry identity");
        }
        if self.lemma.trim().is_empty() || self.lemma.len() > 4096 {
            return Err("invalid headword");
        }
        check_language(self.headword_language.as_deref())?;
        check_language(self.definition_language.as_deref())?;
        for sense in &self.senses {
            validate_sense(sense, 0)?;
        }
        Ok(())
    }
}

fn validate_sense(sense: &Sense, depth: usize) -> Result<(), &'static str> {
    if depth >= 16 {
        return Err("excessively nested senses");
    }
    check_language(sense.definition_language.as_deref())?;
    for child in &sense.children {
        validate_sense(child, depth + 1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_tags_allow_known_and_unknown_values() {
        for tag in ["es", "en", "es-419", "zh-Hant", "und"] {
            assert!(valid_language_tag(tag));
        }
        for tag in ["", "es_", " es", "en--US", "no spaces"] {
            assert!(!valid_language_tag(tag));
        }
        let source = DictionarySource {
            id: "unknown-language".into(),
            title: "Unclassified dictionary".into(),
            format: SourceFormat::Dict,
            origin: None,
            headword_language: None,
            definition_language: None,
            license: None,
            attribution: None,
            metadata: Value::Null,
        };
        assert!(source.validate().is_ok());
    }

    #[test]
    fn nested_senses_and_raw_metadata_round_trip_without_reordering() {
        let entry = LexicalEntry {
            source_id: "wiktextract-en".into(),
            source_entry_id: "es:casa:noun:3".into(),
            lemma: "casa".into(),
            headword_language: Some("es".into()),
            definition_language: Some("en".into()),
            part_of_speech: Some("noun".into()),
            senses: vec![
                Sense {
                    number: Some("1".into()),
                    definition_language: Some("en".into()),
                    glosses: vec!["house".into(), "home".into()],
                    labels: vec!["common".into()],
                    examples: vec![Example {
                        text: "La casa es azul.".into(),
                        translation: Some("The house is blue.".into()),
                        reference: None,
                    }],
                    children: vec![Sense {
                        number: Some("1a".into()),
                        definition_language: Some("en".into()),
                        glosses: vec!["home".into()],
                        labels: vec![],
                        examples: vec![],
                        children: vec![],
                    }],
                },
                Sense {
                    number: Some("2".into()),
                    definition_language: Some("en".into()),
                    glosses: vec!["household".into()],
                    labels: vec![],
                    examples: vec![],
                    children: vec![],
                },
            ],
            pronunciations: vec![Pronunciation {
                ipa: Some("/ˈkasa/".into()),
                audio: None,
                labels: vec!["Spain".into()],
            }],
            etymologies: vec!["From Latin casa.".into()],
            forms: vec![WordForm {
                spelling: "casas".into(),
                tags: vec!["plural".into()],
            }],
            relations: vec![RelatedTerm {
                kind: "synonym".into(),
                term: "hogar".into(),
                language: Some("es".into()),
            }],
            metadata: serde_json::json!({
                "edition": "en",
                "raw-tags": ["historical", {"position": 5}],
                "unrecognized": {"nested": [1, 2, 3]}
            }),
        };
        entry.validate().unwrap();
        let encoded = serde_json::to_vec(&entry).unwrap();
        let decoded: LexicalEntry = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, entry);
        assert_eq!(decoded.senses[0].glosses, ["house", "home"]);
        assert_eq!(decoded.senses[1].glosses, ["household"]);
    }

    #[test]
    fn bad_identity_and_language_are_rejected() {
        let entry = LexicalEntry {
            source_id: "dict".into(),
            source_entry_id: "".into(),
            lemma: "árbol".into(),
            headword_language: Some("es".into()),
            definition_language: Some("en--US".into()),
            part_of_speech: None,
            senses: vec![],
            pronunciations: vec![],
            etymologies: vec![],
            forms: vec![],
            relations: vec![],
            metadata: Value::Null,
        };
        assert!(entry.validate().is_err());
    }
}
