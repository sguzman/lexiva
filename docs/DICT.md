# GNU DICT adapter — current implementation

The adapter lives in `src/dict.rs` with compressed I/O in `src/dict/dictzip.rs` and is **not yet wired into the GUI**. The implementation can stream plain or dictzip records into `DictionaryStore::import_stream`.

## Authoritative format notes

The [dictd database-format manual](https://man.archlinux.org/man/dictd.8) describes a tab-delimited `.index` record with the headword, base-64 encoded byte offset and base-64 byte length. These numbers refer to the uncompressed flat data stream; they are **not** line numbers, UTF-8 character offsets, nor RFC 4648 whole-byte encoded values. The [ArchWiki dictd guide](https://wiki.archlinux.org/title/Dictd) uses paths such as `/usr/share/dictd/eng-spa.index` and `eng-spa.dict.dz`.

## Implemented

- **Non-recursive read-only discovery** of paired `.index`/(`.dict` or `.dict.dz`) resources in `/usr/share/dictd`, `/usr/local/share/dictd` and explicitly selected paths. Absent directories do not cause failure, while unreadable directories produce an error. Discovery order is deterministic.
- **Plain `.dict` and dictzip `.dict.dz`** reading with checked base-64 offsets and lengths, byte-exact definition reads, 16 KiB index-line limit and 4 MiB article limit. Version-1 dictzip `RA` metadata is parsed with length/bounds checks; raw-DEFLATE chunks are decompressed individually with a bounded 64 KiB maximum per chunk and a one-chunk cache.
- Plain-text body preservation as a single sense, attached to the caller-supplied dictionary source, headword language and definition language. Language or grammatical metadata is **never inferred from the filename**. The original index line and byte offset/length are retained.
- Skip `00-database-*` administrative records from headword results. Domain-specific metadata extraction and rich article markup decoding remain unfinished.
- Iterator emits an error and stops on malformed/invalid indexes; a caller using the transactional import API retains the last published dictionary.
- Verified synthetic tests for Unicode words, homographs, index corruption, overflow, discovered files and source-scoped SQLite insertion.

## Limitations and next steps

- Dictzip `.dict.dz` version 1 is implemented and verified with a synthetic gzip-valid `RA`-table fixture. Versions other than v1 and ordinary gzip without `RA` are intentionally refused. Authentic FreeDict files are not yet cross-checked; the gzip member's whole-stream CRC is not calculated by random-access reads, although each chunk and its output length are validated.
- Preserve detailed FreeDict editorial metadata, language direction and article formatting under **LEX-033**.
- Run actual Arch-installed FreeDict/DICT sources through source discovery and the import/search pipeline and compare representative definitions with the input files under **LEX-034**.
- The GUI source manager, filters and definition reader are independent upcoming tasks M05/M06.

**Verification:** [CI run 42835023624](https://github.com/sguzman/lexiva/actions/runs/42835023624): 33 tests, 0 failures, successful formatting, compile and Clippy. Synthetic compression fixture and end-to-end import are covered; this is not real FreeDict-package QA.

## Dictzip implementation contract

The binary fixture `tests/fixtures/synthetic.dict.dz` was constructed from original synthetic example definitions using raw DEFLATE `Z_FULL_FLUSH` per chunk. It is gzip-compatible and contains an unknown extra subfield, a version-1 `RA` table, optional filename, chunk lengths and gzip footer. It does **not** include copyrighted dictionary contents. The reader validates the gzip/RA layout, bounds compressed chunks against the trailer, and enforces the expected inflated output size per chunk. It caches at most one decompressed chunk and never materializes a complete dictionary. This follows [dictzip(1)](https://man.archlinux.org/man/dictzip.1.en) and the [dictd implementation](https://sources.debian.org/src/dictd/1.13.1%2Bdfsg-1/dictzip.c/).
