//! Source-attributed definition presentation.
//! All dictionary text is displayed as plain text: no embedded HTML/scripts.

use std::fmt::Write as _;

use eframe::egui;

use crate::model::Sense;
use crate::storage::SearchHit;

pub fn copy_text(hit: &SearchHit) -> String {
    let entry = &hit.entry;
    let mut output = format!("{}\nSource: {}\n", entry.lemma, hit.source.title);
    if let Some(pos) = &entry.part_of_speech {
        let _ = writeln!(output, "Part of speech: {pos}");
    }
    if let Some(language) = &entry.headword_language {
        let _ = writeln!(output, "Word language: {language}");
    }
    if let Some(language) = &entry.definition_language {
        let _ = writeln!(output, "Definition language: {language}");
    }
    for pronunciation in &entry.pronunciations {
        if let Some(ipa) = &pronunciation.ipa {
            let _ = writeln!(output, "IPA: {ipa}");
        }
    }
    for (position, sense) in entry.senses.iter().enumerate() {
        append_sense(&mut output, sense, &(position + 1).to_string());
    }
    for example in &entry.etymologies {
        let _ = writeln!(output, "Etymology: {example}");
    }
    if let Some(attribution) = &hit.source.attribution {
        let _ = writeln!(output, "Attribution: {attribution}");
    }
    // Keep source-provided newlines; a copied FreeDict article must not
    // lose its trailing whitespace or original line breaks.
    output
}

fn append_sense(output: &mut String, sense: &Sense, index: &str) {
    let number = sense.number.as_deref().unwrap_or(index);
    for gloss in &sense.glosses {
        let _ = writeln!(output, "{number}. {gloss}");
    }
    for example in &sense.examples {
        let _ = writeln!(output, "   Example: {}", example.text);
        if let Some(translation) = &example.translation {
            let _ = writeln!(output, "   Translation: {translation}");
        }
    }
    for (position, child) in sense.children.iter().enumerate() {
        append_sense(output, child, &format!("{number}.{}", position + 1));
    }
}

pub fn show(ui: &mut egui::Ui, hit: &SearchHit, scale: f32) {
    let word = &hit.entry;
    ui.label(egui::RichText::new(&word.lemma).strong().size(28.0 * scale));
    ui.add_space(3.0);
    ui.horizontal_wrapped(|ui| {
        if let Some(pos) = &word.part_of_speech {
            ui.strong(pos);
            ui.separator();
        }
        ui.weak(format!(
            "{} → {}",
            word.headword_language.as_deref().unwrap_or("?"),
            word.definition_language.as_deref().unwrap_or("?")
        ));
    });
    ui.label(
        egui::RichText::new(&hit.source.title)
            .small()
            .color(ui.visuals().weak_text_color()),
    );
    if ui.button("Copy definition").clicked() {
        ui.ctx().copy_text(copy_text(hit));
    }
    ui.add_space(10.0);
    ui.separator();

    if !word.pronunciations.is_empty() {
        ui.label(egui::RichText::new("Pronunciation").strong());
        for pronunciation in &word.pronunciations {
            if let Some(ipa) = &pronunciation.ipa {
                ui.label(egui::RichText::new(ipa).size(16.0 * scale));
            }
            if !pronunciation.labels.is_empty() {
                ui.weak(pronunciation.labels.join(" · "));
            }
        }
        ui.add_space(8.0);
    }

    if !word.etymologies.is_empty() {
        ui.label(egui::RichText::new("Etymology").strong());
        for text in &word.etymologies {
            ui.label(egui::RichText::new(text).size(16.0 * scale));
        }
        ui.add_space(8.0);
    }

    ui.label(egui::RichText::new("Definitions").strong().size(18.0 * scale));
    ui.add_space(5.0);
    if word.senses.is_empty() {
        ui.weak("This source has no parsed definitions for this entry.");
    } else {
        for (index, sense) in word.senses.iter().enumerate() {
            show_sense(ui, sense, &(index + 1).to_string(), scale);
            ui.add_space(8.0);
        }
    }

    if !word.forms.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Forms").strong());
        ui.horizontal_wrapped(|ui| {
            for form in &word.forms {
                ui.label(egui::RichText::new(&form.spelling).size(16.0 * scale));
                if !form.tags.is_empty() {
                    ui.weak(format!("({})", form.tags.join(", ")));
                }
            }
        });
    }
    if !word.relations.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Related words").strong());
        for relation in &word.relations {
            ui.label(egui::RichText::new(format!("{} · {}", relation.kind, relation.term)).size(16.0 * scale));
        }
    }

    ui.add_space(10.0);
    ui.separator();
    if let Some(attribution) = &hit.source.attribution {
        ui.weak(attribution);
    }
    if let Some(license) = &hit.source.license {
        ui.weak(format!("Dictionary license: {license}"));
    } else {
        ui.weak("Dictionary license not specified; check source terms.");
    }
}

fn show_sense(ui: &mut egui::Ui, sense: &Sense, index: &str, scale: f32) {
    let number = sense.number.as_deref().unwrap_or(index);
    if sense.glosses.is_empty() {
        ui.weak(format!("{number}. No gloss in source"));
    } else {
        for (position, gloss) in sense.glosses.iter().enumerate() {
            if position == 0 {
                ui.label(egui::RichText::new(format!("{number}. {gloss}")).size(16.0 * scale));
            } else {
                ui.label(egui::RichText::new(format!("    {gloss}")).size(16.0 * scale));
            }
        }
    }
    if !sense.labels.is_empty() {
        ui.weak(sense.labels.join(" · "));
    }
    for example in &sense.examples {
        ui.label(egui::RichText::new(&example.text).italics().size(15.0 * scale));
        if let Some(translation) = &example.translation {
            ui.weak(translation);
        }
        if let Some(reference) = &example.reference {
            ui.small(reference);
        }
    }
    for (position, child) in sense.children.iter().enumerate() {
        ui.indent(format!("sense-{index}-{}", position + 1), |ui| {
            show_sense(ui, child, &format!("{number}.{}", position + 1), scale);
        });
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::model::{DictionarySource, Example, LexicalEntry, SourceFormat};

    #[test]
    fn copy_preserves_ordered_senses_examples_unicode_and_attribution() {
        let hit = SearchHit {
            source: DictionarySource {
                id: "example".into(),
                title: "Test bilingual dictionary".into(),
                format: SourceFormat::Wiktextract,
                origin: None,
                headword_language: Some("es".into()),
                definition_language: Some("en".into()),
                license: None,
                attribution: Some("Wiktionary contributors".into()),
                metadata: Value::Null,
            },
            entry: LexicalEntry {
                source_id: "example".into(),
                source_entry_id: "1".into(),
                lemma: "árbol".into(),
                headword_language: Some("es".into()),
                definition_language: Some("en".into()),
                part_of_speech: Some("noun".into()),
                senses: vec![
                    Sense {
                        number: Some("1".into()),
                        definition_language: Some("en".into()),
                        glosses: vec!["tree\n".into()],
                        labels: vec![],
                        examples: vec![Example {
                            text: "El árbol es alto.".into(),
                            translation: Some("The tree is tall.".into()),
                            reference: None,
                        }],
                        children: vec![],
                    },
                    Sense {
                        number: Some("2".into()),
                        definition_language: Some("en".into()),
                        glosses: vec!["family tree".into()],
                        labels: vec![],
                        examples: vec![],
                        children: vec![],
                    },
                ],
                pronunciations: vec![],
                etymologies: vec![],
                forms: vec![],
                relations: vec![],
                metadata: Value::Null,
            },
            exact: true,
        };
        let copy = copy_text(&hit);
        assert!(copy.contains("árbol"));
        assert!(copy.contains("Source: Test bilingual dictionary"));
        assert!(copy.contains("1. tree\n\n"));
        assert!(copy.contains("2. family tree"));
        assert!(copy.contains("Example: El árbol es alto."));
        assert!(copy.contains("Translation: The tree is tall."));
        assert!(copy.contains("Attribution: Wiktionary contributors"));
        assert!(copy.find("1. tree").unwrap() < copy.find("2. family tree").unwrap());
    }
}
