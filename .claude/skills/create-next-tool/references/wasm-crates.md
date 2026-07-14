# Proven wasm-safe crates + recipes (wafer wasm32-wasip1 / wasm-pack wasm32-unknown-unknown)

Read this before adding ANY new dependency. An engine crate must INSTANTIATE, not just compile —
building the block wasm (`cargo build --target wasm32-wasip1 --release`; the optional `wafer build`
does the same thing if that CLI is installed) then actually invoking it via `cargo install --path
cli --force && gizza tool <slug> …` is the gate (the CLI embeds the same wafer-run wasmi runtime
chat would use); watch for `cannot find import
wasi_snapshot_preview1::{poll_oneoff,path_open,fd_close}`.

**Crypto / encoding crates that instantiate in wafer (wasm32-wasip1):** `rsa` 0.9 (sign: pkcs1v15 +
pss, `features=["sha2"]`), `p256/p384/p521` (+`spki`+`pkcs8`, EC needs `features=["arithmetic",
"pkcs8"]`), `pgp` 0.14 (rPGP, `default-features=false`; encrypt/sign; for multi-recipient encryption
wrap primary/subkey in a small enum impl'ing `PublicKeyTrait` since the slice must be homogeneous),
`lopdf` 0.36 encryption (`Document::encrypt` + `EncryptionVersion::V5` AES-256, not feature-gated),
`hmac`+`sha1`+`sha2`+`base32` (TOTP), `pem`, `quick-xml` 0.36, `qrcode` 0.14 (`features=["svg"]` → SVG
string, no image dep), `zip` 8 (`default-features=false, features=["deflate"]`, read + write).

**QR decode: use `quircs`, NOT `rqrr`.** `rqrr` compiles to wasm but pulls filesystem WASI imports
(`path_open`/`fd_close`) the wafer runtime doesn't provide → fails to instantiate. `quircs` decodes
from a raw grayscale buffer (`image::load_from_memory(..).to_luma8()` → `Quirc::identify`) and
instantiates fine.

**ECDSA signing (ecdsa-sign):** use `p256`/`p384 = { default-features=false, features=["arithmetic",
"pkcs8","ecdsa","pem"] }` (the `pem` feature is what enables `SigningKey::from_pkcs8_pem`; there is no
`sec1` feature flag) + `ecdsa = { features=["der"] }`. Sign with `use pXXX::ecdsa::{signature::Signer,
Signature, SigningKey}; use pXXX::pkcs8::DecodePrivateKey;` then `let sig: Signature = sk.sign(msg)`
(deterministic RFC-6979 — no RNG, instantiates clean in wafer, no getrandom needed). DER bytes =
`sig.to_der().as_bytes().to_vec()` (NOT `.to_bytes()`); raw r||s = `sig.to_bytes().to_vec()` (64 B P-256,
96 B P-384). **Skip P-521**: `p521`'s ECDSA `Signer` impl is randomized-only (gated on `getrandom`, uses
`OsRng`, no RFC-6979 path), so it breaks determinism and pulls getrandom — support P-256/P-384 only.
Verify DER output cross-tool with `openssl dgst -sha256 -verify pub.pem -signature sig.der msg`.

**Ed25519 (ed25519-key-pair-generator):** `ed25519-dalek = { default-features=false, features=["pkcs8",
"pem","rand_core"] }` + `rand="0.8"`. `SigningKey::generate(&mut rand::rngs::OsRng)`, then
`sk.to_pkcs8_pem(LineEnding::LF)` (import `ed25519_dalek::pkcs8::spki::der::pem::LineEnding` +
`EncodePrivateKey`/`EncodePublicKey`), `vk.to_public_key_pem(...)`, raw via `sk.to_bytes()`/`vk.to_bytes()`
(32 B each). Re-parse with `SigningKey::from_pkcs8_pem` (needs `DecodePrivateKey`). Like key-gen generally
(see generate-rsa-key-pair): **no page** — a zero-input non-deterministic generator doesn't fit the page's
recompute-on-input model. No-input chat tool: empty `#[derive(Deserialize,Default)] #[serde(default)] struct
Args {}`, descriptor `ToolDescriptor::new(Input::None)` (no params) → authored schema is just
`{"type":"object","properties":{},"additionalProperties":false}`.

**Text/parsing crates:** `mail-parser = "0.11"` (default-features=false) — RFC 5322/MIME
email parsing; `MessageParser::default().parse(bytes)`, `msg.from()/to()/cc()` return `Option<&Address>`
(use `.iter()` → `Addr::name()/address()`), `msg.body_text(0)/body_html(0)`, `msg.attachments()` →
`MessagePart` (`use mail_parser::MimeHeaders` for `.attachment_name()/.content_type()`; ContentType has
`.ctype()/.subtype()`), `msg.date().to_rfc3339()`. `htmd = "0.5"` (HTML→Markdown) + `nanohtml2text = "0.1"`
(`html2text(&s)`, HTML→plain) + `quick-xml = "0.40"` (default-features=false) all instantiate in wafer.

**EPUB / ZIP-container parsing (epub-to-markdown):** an EPUB is a ZIP — read with
`zip::ZipArchive::new(Cursor::new(bytes))`, find OPF via `META-INF/container.xml` (`<rootfile full-path>`),
parse the OPF with quick-xml for `<manifest><item id href media-type>` + `<spine><itemref idref>` to get
**reading order** (don't use ZIP/alphabetical order), resolve hrefs relative to the OPF dir, convert each
XHTML. quick-xml: match elements by local name (strip `ns:` prefix), handle both `Event::Start` and
`Event::Empty` for self-closing `<item/>`. Binary-file-in/text-out → **no page** (file-input pattern).

**An "ffmpeg"-tagged image→GIF/animation tool often needs NO ffmpeg** (gif-from-images): the `image` crate
(features incl. `gif`) encodes animated GIFs purely — `use image::codecs::gif::{GifEncoder, Repeat};
enc.set_repeat(Repeat::Infinite); enc.encode_frame(Frame::from_parts(rgba, 0, 0,
Delay::from_numer_denom_ms(ms, 1)))`. Building it pure-Rust makes it run on ALL backends (incl. chat SW),
strictly better than ffmpeg-only. Multi-image-in + image-bytes-out → chat+CLI, **no page** (use
`Param::source_list("images", 1)` + `Vec<SourceFields>`, resolve each via `resolve_source`, like
image-collage/images-to-pdf). Don't reflexively skiplist a media tool as "ffmpeg" — check if `image`/a
pure crate covers it first.

**OpenPGP key generation (generate-pgp-key-pair):** rPGP `pgp = "0.14"` (default-features=false) + rand 0.8
+ chrono(`clock`) + smallvec. `SecretKeyParamsBuilder::default().key_type(KeyType::EdDSALegacy /
Rsa(bits)).can_certify(true).can_sign(true).primary_user_id(uid).subkey(SubkeyParamsBuilder…
.key_type(KeyType::ECDH(ECCCurve::Curve25519) / Rsa(bits)).can_encrypt(true).build()?)` →
`.build()?.generate(&mut OsRng)?` → `.sign(&mut rng, || passphrase)?` = `SignedSecretKey`. Armor:
`sk.to_armored_string(None.into())?`; public = `let pk: SignedPublicKey = sk.into(); pk.to_armored_string(
None.into())?`. Fingerprint: `sk.fingerprint().as_bytes()`. Tests need `use pgp::types::SecretKeyTrait` for
`.unlock()`. Curve25519 = fast (good for tests); RSA-4096 slow. Non-deterministic generator → **no page**.
Cross-verify a generated public key by feeding it to the existing `pgp-encrypt` tool.

**OpenPGP key inspection with a page (pgp-key-info):** `pgp = "0.14"` works for read-only key
metadata on both wafer and wasm-pack, but the browser `wasm32-unknown-unknown` build may pull
`getrandom` transitively through crypto crates even when the code only parses keys. Add
`getrandom = { version = "0.2", features = ["js"] }` in the core crate so
`wasm-pack --target web` doesn't fail with "wasm*-unknown-unknown targets are not supported by
default". The same crate still builds for `wasm32-wasip1` (`cargo build --target wasm32-wasip1
--release`, or `wafer build` if that optional CLI is installed).

**Misc proven crates:** `toml = "0.8"` (config; TOML needs a table root + has no null),
`color_quant = "1"` (NeuQuant palette quantization to N colors), `kamadak-exif = "0.6"` (EXIF/TIFF read —
`Reader::new().read_from_container(Cursor)`, `field.tag`/`field.display_value().with_unit(&exif)`, GPS
rationals → decimal), `serde_json` `preserve_order` feature (keeps object key order). PDF *generation*
needs no new crate — build content streams with the already-proven `lopdf` (base-14 Helvetica, BT/Tf/Td/Tj/ET).
HSL image edits: hand-roll RGB↔HSL (no extra dep) for hue-shift / sat / lightness. For exact RFC test
vectors (e.g. RFC 7638 jwk-thumbprint) WebFetch the RFC rather than trusting memory — a mis-remembered
constant fails the vector.

**HTML tokenizing (html-formatter / html-minifier):** HTML is NOT well-formed XML so quick-xml (used by
xml-formatter) can't parse it. A forgiving quote-aware tag scanner (`scan_tag` skipping quoted `>`) works;
VOID elements don't indent; pre/textarea/script/style are emitted verbatim.

**Writing .xlsx workbooks (csv-to-xlsx):** `rust_xlsxwriter = "0.79"` (default features `[]`; its `zip`
dep uses pure-Rust `deflate`/miniz_oxide) writes real Office Open XML workbooks and INSTANTIATES
under `cargo build --target wasm32-wasip1 --release` (wasm32-wasip1). Native cell types: `write_number`/`write_boolean`/`write_string`(`_with_format`),
`Format::new().set_bold()`, `set_freeze_panes(1,0)`, `worksheet.autofit()`, `workbook.save_to_buffer()`.
**Browser gotcha:** every `Workbook::new()` stamps a creation timestamp via `ExcelDateTime::utc_now()` →
`SystemTime::now()`, which **panics at runtime on wasm32-unknown-unknown** (no std clock) even though it's
fine under wafer's wasi. rust_xlsxwriter's `wasm` feature switches that call to `js_sys::Date::now()`.
Enable it ONLY for the browser build so the wafer/chat build keeps the SystemTime path:
`[target.'cfg(target_arch = "wasm32")'.dependencies] rust_xlsxwriter = { version = "0.79", features = ["wasm"] }`
in `web/Cargo.toml` (the web crate is only ever built by wasm-pack → wasm32-unknown-unknown; feature
unification turns the feature on for the copy of rust_xlsxwriter `core` uses in that build). Verify a
produced workbook by reading it back with `calamine` (dev-dep) or unzipping `xl/sharedStrings.xml` /
`xl/worksheets/sheet1.xml`. Binary output → the page needs `page/custom.js` `renderResult` to turn the
base64 `data:` URL into a Download button (reuse the generator's `#tool-output-download` anchor).
