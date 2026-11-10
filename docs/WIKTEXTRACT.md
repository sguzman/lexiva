# Wiktextract / Kaikki import — implemented adapter and limits

**Status:** LEX-040, LEX-041 and LEX-042 verified against nine synthetic, upstream-schema-shaped tests. Actual downloaded Wiktionary records and corpus-scale benchmarks are **not yet verified** (LEX-043/044).

## Source and edition are not the same as word language

Wiktextract's `lang_code` field identifies the language of an individual headword. The **Wiktionary edition** identifies the language in which its glosses are written, even if the word comes from a different language.

| Explicit source edition | Source word filter | Indexed headword language | Indexed definition language |
|---|---|---|---|
| English Wiktionary | `es` | `es` | `en` |
| Spanish Wiktionary | `es` | `es` | `es` |
| English Wiktionary | `en` | `en` | `en` |

Lexiva never assigns a meaning language based on a headword's spelling or filename. `WiktextractReader::open(path, source, edition)` refuses a mismatch between the explicit edition and the source's declared definition language. `source_from_jsonl` constructs a matching source with a chosen word-language filter. No package manager, server, telemetry or download dependency is involved.

## Files and memory constraints

- Streaming read-only `.jsonl` and `.jsonl.gz` input using standard file I/O and `flate2::MultiGzDecoder`, one logical JSON line at a time.
- Each decompressed input line has a **2 MiB** cap; oversized/corrupt lines stop import safely. A source's **4 MiB serialized-entry** guard still applies at SQLite staging.
- The existing import engine accepts up to **128** mapped records per committed staging batch, with cancellation checks and progress counts for selected entries. A failed or interrupted import cannot replace the previously published source.
- Language filtering occurs during streaming: when `es` is selected, records belonging to other languages are skipped. Every accepted record retains a stable source-local line-number identifier; homographs/POS splits are not collapsed.
- Very large imports also need disk-space preflight, UI progress/error feedback, resource measurement and compatibility with unusually large records before the full 24+ GiB English-edition raw dump should be considered accepted. All of this is LEX-043 and still pending.

## Structured mapping

The adapter populates word, language, POS, ordered senses, gloss lists, tags/topics, usage examples, example translations and references, pronunciation IPA/audio links, inflectional forms, etymology text and common relations (synonyms/antonyms/derived/related/hyper/hyponyms). Gloss chains remain in source order and are not artificially treated as independently verified nested sense trees.

Every record is also stored under `metadata.wiktextract_record` as a structured JSON value, with `wiktionary_edition` and `jsonl_line`, so unknown edition-specific fields, raw glosses, sense IDs and deeper relation structure remain recoverable. **Structured JSON preservation is not a byte-identical backup** of the original source lines.

If a record lacks `lang_code`, fails JSON parsing, or has a missing headword, the importer stops with an error and the previous searchable dictionary remains intact. Future compatibility work must handle real extractor variations explicitly rather than inventing meanings.

## Data provenance and redistribution

The original installed/down­loaded data file remains external and read-only. The source stores its local path and explicit edition, and credits Wiktionary contributors/Wiktextract. `license` is deliberately unset pending per-source redistribution and attribution review. **No Kaikki extract, dictionary payload or font has been copied into this repository.**

Format references:
- [Official Wiktextract format and streaming guidance](https://github.com/tatuylonen/wiktextract)
- [English-edition TypedDict fields](https://github.com/tatuylonen/wiktextract/blob/master/src/wiktextract/extractor/en/type_utils.py)
- [English-edition raw dumps](https://kaikki.org/dictionary/rawdata.html)
- [Spanish-edition raw dumps](https://kaikki.org/eswiktionary/rawdata.html)

## Verification evidence

[CI run 42845015317](https://github.com/sguzman/lexiva/actions/runs/42845015317) passed 45 regular tests (9 new Wiktextract), 1 real FreeDict package test, formatting, compilation, Clippy. Tests include English-edition Spanish words with English definitions, Spanish-edition Spanish words with Spanish definitions, ordered senses, raw metadata round-trip, gzip equivalence, 400-record staged import, incorrect edition refusal, 2 MiB line rejection and transactional rollback.

**Pending:** LEX-043 (large import/progress/cancel/resource evaluation), LEX-044 (authentic extract records/semantic variants). GUI dictionary management and reading are separate M05/M06 work.
