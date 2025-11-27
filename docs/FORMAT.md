# 3DCF Container Formats

## Binary `.3dcf`
- Schema: `proto/3dcf.proto` (compiled via `prost_build`).
- Serialized with Protobuf, then compressed with `zstd` (level 3, multi-threaded when available).
- Cells are delta-encoded along `(z, x, y)` axes for compactness; `code_id` holds the 32-byte `blake3` hash of the normalized cell text.
- `dict` maps `code_id → payload` to guarantee lossless decode.
- `numguards` embed numeric checksum guards for high-value measurements; `3dcf decode` warns whenever the decoded payload disagrees with the recorded checksum/units so numeric integrity regressions are visible immediately.

## JSON `.3dcf.json`
- 1:1 serde representation of the document structure.
- Hashes are hex strings, dict encoded as `[ ["hash", "payload"], ... ]` to preserve ordering.

## Text `.3dcf.txt`
- Prompt-friendly textual serialization implemented in `TextSerializer`.
- Header & footer:
  - `<ctx3d grid=coarse codeset=HASH256 preset=reports budget=256>`
  - Optional `grammar: --select "z=0,x=0..1024,y=0..4096"`
  - `</ctx3d>` closing tag so parsers can sanity check completeness.
- Body lines follow the grammar:
  - `(z=0,x=10,y=20,w=700,h=20,code=0011223344556677,rle=0,imp=120,type=TEXT) "Preview text"`
  - Coordinates are absolute, `code` is the first 16 hex chars of the 32-byte payload hash, previews are JSON-style quoted with `"` escaped.
- Table previews:
  - `auto` mode emits `[csv ...]` snippets for small tables, e.g. `[csv Quarter, Revenue, Cost | Q1, 10, 5]`.
  - For large tables or when `dim` mode is forced: `[table rows=6 cols=4]` summarises dimensions.
- Serializer knobs (via config/CLI) control preview length, table mode, and metadata labels so downstream prompts stay consistent.

### Deduplication knobs

Encoding uses page-level importance scores plus a rolling dedup window (configurable via CLI/TOML) so repeated headers/footers collapse into a single macro-token. The `.3dcf.txt` output therefore maintains reading order while delivering ≥×5 token savings on typical report/slides presets.
