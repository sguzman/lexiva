# Wiktextract / Kaikki import — implemented adapter and limits

**Status:** M04's five roadmap tasks (LEX-040–044) have passed scoped CI acceptance, including original authentic Kaikki per-word JSONL and a 150,000-record generated compressed stress workload. **This does not establish compatibility or performance across a complete 24.8GB Wiktionary raw dump.**

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
- A dedicated CI stress test now measures memory and database growth using 150,000 generated JSONL records, including cancellation and reimport of 120,000 accepted Spanish entries. **Preflight for genuinely insufficient disk space, user-facing import progress, and full 24+ GiB raw dump performance remain unfinished** and are tracked as later GUI/hardening and acceptance concerns.

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

[CI run 42855279348](https://github.com/sguzman/lexiva/actions/runs/42855279348) passed formatting, Rust compilation, strict Clippy, **45 regular tests plus 4 targeted tests (0 failures)**:
- A real installed FreeDict regression in both language directions;
- An isolated 150,000-line, compressed Wiktextract stress test (120,000 Spanish headwords indexed, source filtering and accurate staged progress; cancellation followed by atomic reimport);
- Two raw, independently parsed Wiktextract **Kaikki.org per-word downloads**: [English-edition Spanish `casás`](https://kaikki.org/dictionary/Spanish/meaning/c/ca/cas%C3%A1s.html) (1 record, 606 bytes) and [Spanish-edition Spanish `caso`](https://kaikki.org/eswiktionary/Espa%C3%B1ol/meaning/c/ca/caso.html) (2 records, 9,056 bytes). Their original parsed JSON values were compared against the importer's preserved record metadata, sense order and POS, then searched in SQLite.

#### Measured stress-run metrics

| Metric | CI measurement |
| --- | ---: |
| Generated entries | 150,000 |
| Selected Spanish entries | 120,000 |
| Gzip source size | 2,070,248 bytes |
| SQLite after initial publication | 266,715,136 bytes |
| SQLite after full reimport | 266,797,056 bytes |
| Process peak resident memory (Linux VmHWM) | 15,132 KiB |
| Initial import duration | 23.37 seconds |
| Full reimport duration | 24.13 seconds |

These figures are **one GitHub-hosted Ubuntu runner measurement**, not a speed promise for other hardware. The test generated the input incrementally, kept an existing source searchable when cancellation interrupted a staged replacement, and verified no duplicate count/FTS damage after reimport.

**M04 is verified complete to its defined tests.** Full-scale dumps, unsupported edition variations, disk-space preflight, GUI import experience, native Arch/Hyprland execution and detailed throughput benchmarks remain outstanding in later milestones.
