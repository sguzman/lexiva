//! GNU DICT administrative records and conservative FreeDict language metadata.
//! Never rewrite source files or assume a license based on the filename.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use serde_json::json;

use super::{DictBody, DictFiles, MAX_ARTICLE_BYTES, MAX_INDEX_LINE_BYTES, invalid, parse_row};
use crate::model::{DictionarySource, SourceFormat};

const MAX_ADMIN_ROWS: usize = 64;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DictMetadata {
    /// Original text of the 00-database-* articles, preserved verbatim.
    pub short: Option<String>,
    pub info: Option<String>,
    pub url: Option<String>,
}

fn admin_key(lemma: &str) -> Option<&'static str> {
    match lemma {
        "00-database-short" | "00databaseshort" => Some("short"),
        "00-database-info" => Some("info"),
        "00-database-url" => Some("url"),
        _ => None,
    }
}

fn cleaned_short_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("00-database-short")
        .or_else(|| trimmed.strip_prefix("00databaseshort"))
        .unwrap_or(trimmed)
        .trim();
    if stripped.is_empty() || stripped.eq_ignore_ascii_case("unknown") {
        None
    } else {
        Some(stripped.to_owned())
    }
}

/// Inspect only the first 64 index rows, which contain dictfmt's reserved
/// administrative entries. Unknown or missing metadata is left unknown.
pub fn inspect_metadata(files: &DictFiles) -> io::Result<DictMetadata> {
    let mut index = BufReader::new(File::open(&files.index)?);
    let file = File::open(&files.data)?;
    let mut data = if files.compressed {
        DictBody::Zipped(super::dictzip::DictzipReader::open(file)?)
    } else {
        let len = file.metadata()?.len();
        DictBody::Plain { file, len }
    };
    let mut metadata = DictMetadata::default();
    for _ in 0..MAX_ADMIN_ROWS {
        let mut line = Vec::new();
        let read = index
            .by_ref()
            .take(MAX_INDEX_LINE_BYTES + 1)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        if line.len() as u64 > MAX_INDEX_LINE_BYTES {
            return Err(invalid("DICT index metadata line exceeds limit"));
        }
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let text =
            std::str::from_utf8(&line).map_err(|_| invalid("invalid metadata index UTF-8"))?;
        let row = parse_row(text)?;
        if row.lemma.is_empty() {
            // FreeDict exports can begin with an index row with no keyword.
            // This is neither metadata nor a lexical entry. Validate its
            // range so corruption is not silently excused, then scan onward.
            if row.length > MAX_ARTICLE_BYTES
                || row
                    .start
                    .checked_add(row.length)
                    .is_none_or(|end| end > data.len())
            {
                return Err(invalid("DICT empty-headword row exceeds data bounds"));
            }
            continue;
        }
        if let Some(key) = admin_key(row.lemma) {
            if row.length > MAX_ARTICLE_BYTES
                || row
                    .start
                    .checked_add(row.length)
                    .is_none_or(|end| end > data.len())
            {
                return Err(invalid("DICT administrative article exceeds data bounds"));
            }
            let raw = String::from_utf8(data.read_at(row.start, row.length)?)
                .map_err(|_| invalid("DICT metadata article is not UTF-8"))?;
            match key {
                "short" if metadata.short.is_none() => metadata.short = Some(raw),
                "info" if metadata.info.is_none() => metadata.info = Some(raw),
                "url" if metadata.url.is_none() => metadata.url = Some(raw),
                _ => (),
            }
        } else if !row.lemma.starts_with("00-database-") {
            // Reserved database rows are conventionally sorted first.
            break;
        }
    }
    Ok(metadata)
}

fn known_language(tag: &str) -> Option<&str> {
    // ISO 639-3 codes are valid BCP-47 primary subtags; normalize common
    // two-letter counterparts to cooperate with Wiktextract later.
    Some(match tag {
        "eng" => "en",
        "spa" => "es",
        "deu" => "de",
        "fra" => "fr",
        "ita" => "it",
        "por" => "pt",
        "nld" => "nl",
        "cat" => "ca",
        "rus" => "ru",
        "pol" => "pl",
        "swe" => "sv",
        "jpn" => "ja",
        "zho" => "zh",
        "ces" => "cs",
        "dan" => "da",
        "fin" => "fi",
        "nor" => "no",
        "ell" => "el",
        "tur" => "tr",
        "ara" => "ar",
        _ => return None,
    })
}

fn pair_from_name(index: &Path, metadata: &DictMetadata) -> Option<(String, String, &'static str)> {
    let basename = index.file_stem()?.to_str()?;
    let (pair, evidence) = if let Some(stripped) = basename.strip_prefix("freedict-") {
        (stripped, "freedict-filename")
    } else if metadata
        .short
        .as_deref()
        .or(metadata.info.as_deref())
        .is_some_and(|raw| raw.to_ascii_lowercase().contains("freedict"))
    {
        (basename, "freedict-administrative-header-and-filename")
    } else {
        return None;
    };
    let (from, to) = pair.split_once('-')?;
    if from.len() != 3
        || to.len() != 3
        || !from.bytes().all(|c| c.is_ascii_lowercase())
        || !to.bytes().all(|c| c.is_ascii_lowercase())
    {
        return None;
    }
    // Unknown 3-letter codes stay *unknown*, not silently invented.
    let source = known_language(from)?;
    let target = known_language(to)?;
    Some((source.to_owned(), target.to_owned(), evidence))
}

/// Create a source record from a read-only DICT file pair. Language direction
/// is assigned only when the recognized FreeDict naming convention and
/// known ISO 639-3 language codes provide evidence. Licenses remain unknown
/// unless the user or a trusted external metadata source supplies them.
pub fn source_from_files(files: &DictFiles, id: &str) -> io::Result<DictionarySource> {
    let metadata = inspect_metadata(files)?;
    let pair = pair_from_name(&files.index, &metadata);
    let fallback = files
        .index
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("invalid DICT file basename"))?;
    let source = DictionarySource {
        id: id.to_owned(),
        title: metadata
            .short
            .as_deref()
            .and_then(cleaned_short_name)
            .unwrap_or_else(|| fallback.to_owned()),
        format: SourceFormat::Dict,
        origin: Some(files.index.to_string_lossy().into_owned()),
        headword_language: pair.as_ref().map(|(from, _, _)| from.clone()),
        definition_language: pair.as_ref().map(|(_, to, _)| to.clone()),
        license: None,
        attribution: None,
        metadata: json!({
            "dict": {
                "admin_short_raw": metadata.short,
                "admin_info_raw": metadata.info,
                "admin_url_raw": metadata.url,
                "index_path": files.index.to_string_lossy(),
                "data_path": files.data.to_string_lossy(),
                "compression": if files.compressed { "dictzip" } else { "none" },
                "language_evidence": pair.as_ref().map(|(_, _, method)| method)
            }
        }),
    };
    source
        .validate()
        .map_err(|reason| invalid(format!("invalid dictionary source: {reason}")))?;
    Ok(source)
}

/// Only decode unambiguous conventional part-of-speech labels in the header.
/// The full original text remains intact in the sense's gloss regardless.
pub(super) fn article_part_of_speech(lemma: &str, article: &str) -> Option<String> {
    let line = article.lines().next()?;
    let rest = line.strip_prefix(lemma)?.trim_start();
    let label = rest.split_whitespace().next_back()?;
    let normalized = match label {
        "n" | "n." => "noun",
        "v" | "v." => "verb",
        "adj" | "adj." => "adjective",
        "adv" | "adv." => "adverb",
        "pron" | "pron." => "pronoun",
        "prep" | "prep." => "preposition",
        "interj" | "interj." => "interjection",
        _ => return None,
    };
    Some(normalized.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("lexiva-meta-{}-{id}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn files(&self, name: &str, index: &str, data: &str) -> DictFiles {
            let index_path = self.0.join(format!("{name}.index"));
            let data_path = self.0.join(format!("{name}.dict"));
            fs::write(&index_path, index).unwrap();
            fs::write(&data_path, data).unwrap();
            DictFiles {
                index: index_path,
                data: data_path,
                compressed: false,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn encode(mut number: u64) -> String {
        const DIGITS: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        if number == 0 {
            return "A".into();
        }
        let mut encoded = Vec::new();
        while number != 0 {
            encoded.push(DIGITS[(number % 64) as usize] as char);
            number /= 64;
        }
        encoded.into_iter().rev().collect()
    }

    #[test]
    fn preserves_admin_articles_and_infers_only_evidenced_language_pairs() {
        // DICT base64 offsets: A=0, S=18, gh=2081 etc. Build a
        // two-record source with single-digit indices for clarity.
        let fixture = Fixture::new();
        let short = "Spanish - English FreeDict";
        let data = format!("{short}\nFound at https://freedict.org/\n");
        let data_bytes = data.as_bytes();
        let start_second = short.len() + 1;
        let index = format!(
            "00-database-short\tA\t{}\n00-database-url\t{}\t{}\n",
            encode(short.len() as u64),
            encode(start_second as u64),
            encode((data_bytes.len() - start_second) as u64)
        );
        let files = fixture.files("freedict-spa-eng", &index, &data);
        let metadata = inspect_metadata(&files).unwrap();
        assert_eq!(metadata.short.as_deref(), Some(short));
        let source = source_from_files(&files, "package-es-en").unwrap();
        assert_eq!(source.title, short);
        assert_eq!(source.headword_language.as_deref(), Some("es"));
        assert_eq!(source.definition_language.as_deref(), Some("en"));
        assert_eq!(source.license, None);
        assert_eq!(
            source.metadata["dict"]["admin_url_raw"],
            "Found at https://freedict.org/\n"
        );
    }

    #[test]
    fn metadata_scanning_skips_empty_headword_and_rejects_bad_ranges() {
        let fixture = Fixture::new();
        let files = fixture.files(
            "eng-spa",
            "\tA\tB\n00-database-short\tA\tB\nword\tA\tB\n",
            "x",
        );
        let info = inspect_metadata(&files).unwrap();
        assert_eq!(info.short.as_deref(), Some("x"));
        let broken = fixture.files("bad", "\tB\tB\nword\tA\tB\n", "x");
        assert!(inspect_metadata(&broken).is_err());
    }

    #[test]
    fn unknown_and_arbitrary_dictionary_names_do_not_invent_languages() {
        let fixture = Fixture::new();
        let files = fixture.files("personal-eng-spa", "casa\tA\tB\n", "x");
        let source = source_from_files(&files, "personal").unwrap();
        assert_eq!(source.headword_language, None);
        assert_eq!(source.definition_language, None);
        assert_eq!(source.title, "personal-eng-spa");
        assert_eq!(source.license, None);
    }

    #[test]
    fn part_of_speech_extraction_preserves_ambiguous_header() {
        assert_eq!(
            article_part_of_speech("poem", "poem [pouim] n\n    poema\n").as_deref(),
            Some("noun")
        );
        assert_eq!(
            article_part_of_speech("poem", "poem n.\n    poema\n").as_deref(),
            Some("noun")
        );
        assert_eq!(article_part_of_speech("casa", "la casa n"), None);
        assert_eq!(article_part_of_speech("casa", "casa azul"), None);
        assert_eq!(article_part_of_speech("casa", "casa"), None);
    }
}
