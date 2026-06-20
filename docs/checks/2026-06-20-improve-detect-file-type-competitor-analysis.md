# detect-file-type — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/detect-file-type` — identify a file's true format from its
leading bytes (magic numbers), independent of name/extension. Chat + CLI (no
page: a file→text report fits neither the pure-text nor the ffmpeg file→media
page shape).

## What competitors do

- **`file(1)` / libmagic** — the reference implementation. Huge magic database,
  reports a free-text description + (with `-i`) a MIME type. Strength: breadth.
  Weakness: free-text output is hard to consume programmatically; needs the
  ~5 MB magic db.
- **Online "what is my file" / "file type checker" sites** (e.g.
  checkfiletype.com, toolslick, aconvert's detector) — upload a file, get a
  format name + MIME. Most only cover ~30–50 common types and several just read
  the extension, which defeats the purpose for renamed files.
- **Rust crates** — `infer` (curated magic table, ~_no_-std, returns
  type+mime+ext), `tree_magic_mini` (libmagic-style db). `infer` is the closest
  analogue to this tool.

## How this tool competes / improves

1. **Structured, machine-readable output** — returns `mime`, `extension`,
   `kind`, `category`, and `bytes` as flat JSON the LLM (or a downstream tool)
   can act on directly, rather than `file`'s free text.
2. **Extension-mismatch flag** — when a source filename's extension disagrees
   with the detected type (a renamed `.jpg` that is really a PDF, malware-style
   `.txt` that is really a PE), it emits an `extension_mismatch` note. Aliases
   (jpg/jpeg, htm/html, docx/zip, …) are treated as consistent so it doesn't
   cry wolf. This is the security/forensics angle most online checkers miss.
3. **ZIP-payload disambiguation** — a bare `PK\x03\x04` is resolved to the
   specific OOXML / OpenDocument / EPUB type by peeking the first entry name
   (`word/`→docx, `xl/`→xlsx, `ppt/`→pptx, stored `mimetype`→epub/odt/ods),
   not just reported as "ZIP".
4. **ISO-BMFF brand resolution** — `ftyp` containers are split by major brand
   into mp4 / mov / m4a / heic / avif / 3gp rather than lumped as "MP4".
5. **Text-format sniffing** — beyond binary magic, recognises XML, SVG (incl.
   xml-prologued), HTML, JSON (with a balance check so prose starting with `{`
   stays plain text), PostScript, and BOM-prefixed UTF-8.
6. **Coarse `category`** bucket (image/video/audio/document/archive/font/
   executable/database/text/data) lets a caller branch without a big match.
7. **Zero dependencies, wasm-safe** — a curated magic table (no libmagic db, no
   non-wasm transitive deps), so it runs on every backend including the chat
   Service Worker.

## Coverage

~50 formats across images (png/jpg/gif/bmp/tiff/ico/psd/webp/heic/avif/svg),
audio (mp3/aac/flac/ogg/wav/midi/aiff/m4a), video (mp4/mov/mkv/webm/avi/flv/
wmv/mpeg/3gp), documents (pdf/rtf/legacy-office/ooxml/opendocument/epub/html/
postscript), archives (zip/gz/bz2/xz/7z/rar/zstd/lz4/tar/ar), fonts (ttf/otf/
woff/woff2), executables (elf/pe/mach-o/wasm/java-class), SQLite, and text.

## Tests

10 core unit tests (one per family + category + unknown-binary fallback +
JSON/text edge cases) + the block drift-guard schema test. CLI verified over
the wire against a PDF, GIF, SVG, and PNG URL — all detected correctly with the
right mime/extension/category/filename.
