# Lexiva — Dictionary format support contract

| Format | Scope | Details |
|---|---|---|
| GNU DICT / dictd | Essential | Read `.index`, `.dict` and random-access dictzip `.dict.dz` offline; no server required. |
| FreeDict via Arch | Essential | Discover package-installed DICT resources read-only; preserve source metadata and per-dictionary license. |
| Wiktextract JSONL / JSONL.gz | Essential | Stream large datasets; preserve edition/headword language and definition language, senses, examples, IPA, etymologies. |
| StarDict | Full release | `.ifo`, `.idx`, `.dict`, compressed variants and optional synonym index; safely render supported markup. |
| FreeDict TEI XML | Full release | Namespace-aware structured entry/sense import, bounded XML handling. |
| Hunspell | Excluded | Spellchecker/morphology resource, not definitions; later optional approval. |
| MDict / DSL / SLOB / ZIM | Excluded | Requires separate qualification and explicit scope decision. |

Recognizing a file extension alone is not compatibility. A supported source must pass real-entry cross-checks, preserve provenance and report unsupported data faithfully. Dictionary content licenses differ from Lexiva's MIT source; nothing is redistributed by default.

## Implemented status — GNU DICT (November 2026)

The adapter in `src/dict.rs` can discover matching `.index` + `.dict` or `.dict.dz` files in `/usr/share/dictd`, `/usr/local/share/dictd` or user-supplied directories **without a dictd server**. Only **uncompressed** `.dict` payloads are readable today; `.dict.dz` pairs are detected but correctly return an unsupported-format error. Synthetic `.index`/`.dict` fixtures pass base-64 offset, UTF-8, homograph, corruption and transactional-import tests. No actual FreeDict package data has yet been cross-checked. See [GNU DICT adapter notes](DICT.md).
