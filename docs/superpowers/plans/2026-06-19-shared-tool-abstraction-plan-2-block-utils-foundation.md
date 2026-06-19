# Shared Tool Abstraction — Plan 2: block-utils foundation (descriptor + helpers)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the single-source `ToolDescriptor` (and its surface derivations) plus the shared skill/media helper layer to `block-utils`, so a tool becomes "declare a descriptor + delegate to `core`" — no per-tool wrapper, schema, or media-orchestration copy-paste.

**Architecture:** New `block-utils` module `descriptor.rs` holding `Input`/`ParamKind`/`Param`/`ToolDescriptor` (builders + `serde` + `to_schema_json()`). New helpers in `lib.rs`: `run_skill` + `respond_ok` (text shape), the media trio `resolve_source` + `dispatch_ffmpeg` + the pure `build_media_envelope`, and a shared `ArgvPlan` struct for ffmpeg page exports (a generic web-export macro proved not worth it — Task 5). This crate is consumed (path/git dep) by every tool; nothing here uses the wasm ABI directly except the wasm-gated host wrappers.

**Tech Stack:** Rust, `serde`/`serde_json`, `thiserror`, `base64`, `wafer-sdk`/`wafer-block` (host calls wasm-gated).

**Spec:** `docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md` §1, §4 (and the query-param §6 consumes `descriptor.json`, produced here).

## Global Constraints

- **Repo:** `gizza-ai`, crate `gizza-ai-block-utils` (`block-utils/`). Additive only — do not change existing public items' behavior (existing tools still compile against them until retrofit).
- **No drift:** logical params are declared once in the descriptor; `to_schema_json()` is the single chat-schema source. Param `name` == chat-schema property == URL query-param name.
- **Error consistency:** helpers return `Result<Vec<u8>, SkillError>`; the tool's `handle()` wraps `Ok(v) => GuestResult::respond(v)`, `Err(e) => GuestResult::error(e.into())`. (This standardizes on proper error signaling; the retrofit will drop url-encode's `{ "error": … }`-as-200 path.)
- **Native-testable core:** descriptor logic, `to_schema_json`, `run_skill`, `respond_ok`, and `build_media_envelope` are native (`cargo test`). Host-calling helpers (`resolve_source`, `dispatch_ffmpeg`) are `#[cfg(target_arch = "wasm32")]` and are build-verified here, behavior-verified by the first media-tool retrofit (Plan 4).
- **CI:** gizza CI = `cargo test` in `block-utils` is **not** a separate CI job today; this crate is built transitively. Add `cargo test --manifest-path block-utils/Cargo.toml` to `.github/workflows/test.yml` as part of Task 1 so the foundation is gated.
- **wafer-run:** builds against the LOCAL wafer-run tree (`.cargo` patch) which already has Plan 1; no new wafer-run dependency is used by this plan (the expression-`parameters` feature is exercised in Plan 4).

---

### Task 1: Descriptor types (`Input`, `ParamKind`, `Param`, `ToolDescriptor`)

**Files:**
- Create: `block-utils/src/descriptor.rs`
- Modify: `block-utils/src/lib.rs` (add `pub mod descriptor; pub use descriptor::*;` near the top, after the crate doc comment)
- Modify: `.github/workflows/test.yml` (add a block-utils test step)
- Test: in `block-utils/src/descriptor.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `Input`, `ParamKind`, `Param`, `ToolDescriptor` with the builder API below. Consumed by Task 2 (`to_schema_json`), Task 4/5, the generator (Plan 3, via `serde` JSON), and every tool's `core::descriptor()`.

- [ ] **Step 1: Write the failing test** — create `block-utils/src/descriptor.rs` with the test first:

```rust
//! Single-source tool descriptor. One declaration per tool (in its `core`
//! crate) from which the chat schema, page form, `build_argv` keying, and the
//! URL query-param contract are all derived — see
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

use serde::{Deserialize, Serialize};

/// The binary/remote input a tool consumes. Varies by surface: chat/CLI take
/// `url`⊕`ref`, the page takes a file upload or `?url=`. Plain text is a
/// `String` [`Param`], not an `Input`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Input {
    None,
    Image,
    Video,
    Document,
    File,
}

/// A logical parameter's type. Numeric bounds live on [`Param::minimum`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    String,
    Integer,
    Number,
    Enum(Vec<String>),
    Bool,
}

/// One logical parameter. `name` is the chat-schema property name, the page
/// field name, AND the URL query-param name (single source, no drift).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub kind: ParamKind,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub multiline: bool,
}

impl Param {
    fn new(name: &str, kind: ParamKind) -> Self {
        Param {
            name: name.to_string(),
            kind,
            required: false,
            default: None,
            minimum: None,
            description: String::new(),
            label: None,
            placeholder: None,
            multiline: false,
        }
    }
    pub fn string(name: &str) -> Self {
        Self::new(name, ParamKind::String)
    }
    pub fn integer(name: &str) -> Self {
        Self::new(name, ParamKind::Integer)
    }
    pub fn number(name: &str) -> Self {
        Self::new(name, ParamKind::Number)
    }
    pub fn boolean(name: &str) -> Self {
        Self::new(name, ParamKind::Bool)
    }
    pub fn enumv<const N: usize>(name: &str, variants: [&str; N]) -> Self {
        Self::new(
            name,
            ParamKind::Enum(variants.iter().map(|s| s.to_string()).collect()),
        )
    }
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub fn default(mut self, v: impl Into<serde_json::Value>) -> Self {
        self.default = Some(v.into());
        self
    }
    pub fn min(mut self, n: f64) -> Self {
        self.minimum = Some(n);
        self
    }
    pub fn describe(mut self, s: &str) -> Self {
        self.description = s.to_string();
        self
    }
    pub fn label(mut self, s: &str) -> Self {
        self.label = Some(s.to_string());
        self
    }
    pub fn placeholder(mut self, s: &str) -> Self {
        self.placeholder = Some(s.to_string());
        self
    }
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }
}

/// One declaration per tool. See module docs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub input: Input,
    pub params: Vec<Param>,
}

impl ToolDescriptor {
    pub fn new(input: Input) -> Self {
        ToolDescriptor {
            input,
            params: Vec::new(),
        }
    }
    pub fn param(mut self, p: Param) -> Self {
        self.params.push(p);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_descriptor_with_typed_params() {
        let d = ToolDescriptor::new(Input::Image)
            .param(Param::integer("width").min(1.0).label("Width (px)"))
            .param(
                Param::enumv("fit", ["contain", "cover", "stretch"])
                    .default("contain")
                    .describe("How to fit."),
            );
        assert_eq!(d.input, Input::Image);
        assert_eq!(d.params.len(), 2);
        assert_eq!(d.params[0].name, "width");
        assert_eq!(d.params[0].kind, ParamKind::Integer);
        assert_eq!(d.params[0].minimum, Some(1.0));
        assert_eq!(d.params[1].default, Some(serde_json::json!("contain")));
        assert_eq!(
            d.params[1].kind,
            ParamKind::Enum(vec!["contain".into(), "cover".into(), "stretch".into()])
        );
    }

    #[test]
    fn descriptor_round_trips_through_json() {
        // The generator (Plan 3) reads an emitted descriptor.json — serde must
        // round-trip losslessly.
        let d = ToolDescriptor::new(Input::None)
            .param(Param::string("expression").required().placeholder("2 + 2"));
        let json = serde_json::to_string(&d).expect("serialize");
        let back: ToolDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(d, back);
    }
}
```

- [ ] **Step 2: Wire the module + verify the test fails**

Add to `block-utils/src/lib.rs` after the crate-level doc comment / first `use`:

```rust
pub mod descriptor;
pub use descriptor::*;
```

Run: `cargo test --manifest-path block-utils/Cargo.toml descriptor:: 2>&1 | tail -20`
Expected: at first this **fails to compile** only if the module wiring is missing; once wired, the two tests compile and **pass** (this task is pure data types, so RED is the pre-wiring compile error). If both pass immediately after wiring, that is acceptable for plain data-type scaffolding — the meaningful RED/GREEN cycles are Tasks 2–5.

- [ ] **Step 3: Add the CI gate**

In `.github/workflows/test.yml`, alongside the existing `cargo test` steps, add:

```yaml
      - name: block-utils tests
        run: cargo test --manifest-path block-utils/Cargo.toml
```

- [ ] **Step 4: Run + commit**

Run: `cargo test --manifest-path block-utils/Cargo.toml descriptor:: 2>&1 | tail -10`
Expected: `test result: ok. 2 passed`.

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add block-utils/src/descriptor.rs block-utils/src/lib.rs .github/workflows/test.yml
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(block-utils): ToolDescriptor/Param/Input descriptor types

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: `ToolDescriptor::to_schema_json()` — derive the chat schema

**Files:**
- Modify: `block-utils/src/descriptor.rs`
- Test: same file's `mod tests`

**Interfaces:**
- Consumes: `ToolDescriptor`, `Param`, `ParamKind`, `Input` (Task 1).
- Produces: `pub fn to_schema_json(&self) -> String` — the JSON Schema string a tool passes to `#[wafer_block(skill(parameters = …))]` (Plan 1's expression form). Media inputs emit a `url`⊕`ref` `oneOf`; logical params become typed properties; required params go in `required`.

- [ ] **Step 1: Write the failing tests**

Add to `block-utils/src/descriptor.rs` `mod tests`:

```rust
    #[test]
    fn schema_for_pure_text_tool() {
        // calculator: Input::None + one required string param.
        let d = ToolDescriptor::new(Input::None)
            .param(Param::string("expression").required().describe("The expression."));
        let v: serde_json::Value =
            serde_json::from_str(&d.to_schema_json()).expect("valid JSON schema");
        assert_eq!(v["type"], "object");
        assert_eq!(v["properties"]["expression"]["type"], "string");
        assert_eq!(v["properties"]["expression"]["description"], "The expression.");
        assert_eq!(v["required"], serde_json::json!(["expression"]));
        assert!(v.get("oneOf").is_none(), "no url/ref oneOf for Input::None");
    }

    #[test]
    fn schema_for_media_tool_has_url_ref_oneof_and_typed_params() {
        // image-resize: Input::Image + optional integer(min) + enum(default).
        let d = ToolDescriptor::new(Input::Image)
            .param(Param::integer("width").min(1.0).describe("Target width in pixels."))
            .param(Param::integer("height").min(1.0).describe("Target height in pixels."))
            .param(
                Param::enumv("fit", ["contain", "cover", "stretch"])
                    .default("contain")
                    .describe("Resize mode."),
            );
        let v: serde_json::Value =
            serde_json::from_str(&d.to_schema_json()).expect("valid JSON schema");
        // url/ref properties + exclusive oneOf.
        assert_eq!(v["properties"]["url"]["type"], "string");
        assert_eq!(v["properties"]["ref"]["type"], "string");
        assert_eq!(
            v["oneOf"],
            serde_json::json!([{ "required": ["url"] }, { "required": ["ref"] }])
        );
        // typed params.
        assert_eq!(v["properties"]["width"]["type"], "integer");
        assert_eq!(v["properties"]["width"]["minimum"], 1.0);
        assert_eq!(v["properties"]["fit"]["enum"], serde_json::json!(["contain", "cover", "stretch"]));
        assert_eq!(v["properties"]["fit"]["default"], "contain");
        // optional params => no top-level required.
        assert!(v.get("required").is_none());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --manifest-path block-utils/Cargo.toml descriptor::tests::schema 2>&1 | tail -15`
Expected: FAIL — `no method named to_schema_json found for ToolDescriptor`.

- [ ] **Step 3: Implement `to_schema_json`**

Add to `impl ToolDescriptor` in `block-utils/src/descriptor.rs`:

```rust
    /// Render the chat-schema JSON for `#[wafer_block(skill(parameters = …))]`.
    /// Single source: properties/enums/defaults/required and the media
    /// `url`⊕`ref` `oneOf` are all derived from this descriptor.
    pub fn to_schema_json(&self) -> String {
        use serde_json::{json, Map, Value};

        let mut properties = Map::new();

        // Media/file/document tools take exactly one of `url` / `ref`.
        let media_label = match self.input {
            Input::Image => Some("Image"),
            Input::Video => Some("Video"),
            Input::Document => Some("Document"),
            Input::File => Some("File"),
            Input::None => None,
        };
        if let Some(label) = media_label {
            properties.insert(
                "url".into(),
                json!({ "type": "string",
                        "description": format!("{label} URL (HTTP/HTTPS). Use either url or ref.") }),
            );
            properties.insert(
                "ref".into(),
                json!({ "type": "string",
                        "description": "Reference id from a prior tool call. Use either url or ref." }),
            );
        }

        let mut required: Vec<Value> = Vec::new();
        for p in &self.params {
            let mut prop = Map::new();
            match &p.kind {
                ParamKind::String => {
                    prop.insert("type".into(), json!("string"));
                }
                ParamKind::Integer => {
                    prop.insert("type".into(), json!("integer"));
                }
                ParamKind::Number => {
                    prop.insert("type".into(), json!("number"));
                }
                ParamKind::Bool => {
                    prop.insert("type".into(), json!("boolean"));
                }
                ParamKind::Enum(variants) => {
                    prop.insert("type".into(), json!("string"));
                    prop.insert("enum".into(), json!(variants));
                }
            }
            if let Some(m) = p.minimum {
                prop.insert("minimum".into(), json!(m));
            }
            if let Some(d) = &p.default {
                prop.insert("default".into(), d.clone());
            }
            if !p.description.is_empty() {
                prop.insert("description".into(), json!(p.description));
            }
            properties.insert(p.name.clone(), Value::Object(prop));
            if p.required {
                required.push(json!(p.name));
            }
        }

        let mut schema = Map::new();
        schema.insert("type".into(), json!("object"));
        schema.insert("properties".into(), Value::Object(properties));
        if !required.is_empty() {
            schema.insert("required".into(), Value::Array(required));
        }
        if media_label.is_some() {
            schema.insert(
                "oneOf".into(),
                json!([{ "required": ["url"] }, { "required": ["ref"] }]),
            );
        }
        Value::Object(schema).to_string()
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --manifest-path block-utils/Cargo.toml descriptor::tests::schema 2>&1 | tail -10`
Expected: `test result: ok. 2 passed`.

- [ ] **Step 5: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add block-utils/src/descriptor.rs
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(block-utils): ToolDescriptor::to_schema_json (single-source chat schema)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: `run_skill` + `respond_ok` (text-shape helper)

**Files:**
- Modify: `block-utils/src/lib.rs` (add at the end, before `#[cfg(test)]` if present, else append a test module)
- Test: `block-utils/src/lib.rs` `#[cfg(test)] mod helper_tests`

**Interfaces:**
- Consumes: `SkillError`, `SkillResultExt` (existing in lib.rs).
- Produces:
  - `pub fn respond_ok<T: Serialize>(value: &T) -> Result<Vec<u8>, SkillError>` — serializes `{ "result": value }`.
  - `pub fn run_skill<A, T, F>(body: &[u8], block: &str, f: F) -> Result<Vec<u8>, SkillError>` where `A: serde::de::DeserializeOwned, T: Serialize, F: FnOnce(A) -> Result<T, SkillError>` — deserialize args (labeled via `block`), call `f`, shape `{result}`.
  - Tool usage (documented, applied in Plan 4): `match run_skill(&body, "url-encode", |a: Args| core::convert(a)) { Ok(v) => GuestResult::respond(v), Err(e) => GuestResult::error(e.into()) }`.

- [ ] **Step 1: Write the failing tests**

Append to `block-utils/src/lib.rs`:

```rust
#[cfg(test)]
mod helper_tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    struct EchoArgs {
        text: String,
    }
    #[derive(Serialize)]
    struct EchoOut {
        echo: String,
    }

    #[test]
    fn run_skill_shapes_result_on_ok() {
        let body = br#"{"text":"hi"}"#;
        let out = run_skill(body, "echo", |a: EchoArgs| {
            Ok::<_, SkillError>(EchoOut { echo: a.text })
        })
        .expect("ok path");
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["result"]["echo"], "hi");
    }

    #[test]
    fn run_skill_labels_bad_json_as_invalid_args() {
        let body = br#"{ not json"#;
        let err = run_skill(body, "echo", |a: EchoArgs| {
            Ok::<_, SkillError>(EchoOut { echo: a.text })
        })
        .expect_err("bad json must error");
        assert!(matches!(err, SkillError::InvalidArgs(_)));
        assert!(err.to_string().contains("invalid echo args"));
    }

    #[test]
    fn run_skill_propagates_inner_error() {
        let body = br#"{"text":"x"}"#;
        let err = run_skill(body, "echo", |_a: EchoArgs| {
            Err::<EchoOut, _>(SkillError::InvalidArgs("nope".into()))
        })
        .expect_err("inner error propagates");
        assert!(matches!(err, SkillError::InvalidArgs(_)));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --manifest-path block-utils/Cargo.toml helper_tests 2>&1 | tail -15`
Expected: FAIL — `cannot find function run_skill` / `respond_ok`.

- [ ] **Step 3: Implement the helpers**

Append to `block-utils/src/lib.rs` (module scope, not inside the test module):

```rust
// ---------------------------------------------------------------------------
// Text-shape skill helper. Returns Result<Vec<u8>, SkillError>; the tool's
// wasm `handle()` wraps Ok => GuestResult::respond, Err => GuestResult::error.
// ---------------------------------------------------------------------------

/// Serialize a success payload as `{ "result": <value> }`.
pub fn respond_ok<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, SkillError> {
    serde_json::to_vec(&serde_json::json!({ "result": value }))
        .map_err(|e| SkillError::Serialize(format!("serialize result: {e}")))
}

/// Run a text-shape skill: parse `A` from `body` (errors labeled
/// `invalid <block> args: …`), call `f`, and shape `{ "result": … }`.
pub fn run_skill<A, T, F>(body: &[u8], block: &str, f: F) -> Result<Vec<u8>, SkillError>
where
    A: serde::de::DeserializeOwned,
    T: serde::Serialize,
    F: FnOnce(A) -> Result<T, SkillError>,
{
    let args: A = serde_json::from_slice(body).invalid_args(block)?;
    let out = f(args)?;
    respond_ok(&out)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --manifest-path block-utils/Cargo.toml helper_tests 2>&1 | tail -10`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 5: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add block-utils/src/lib.rs
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(block-utils): run_skill + respond_ok text-shape helper

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Media helpers — pure `build_media_envelope` + wasm `resolve_source`/`dispatch_ffmpeg`

**Files:**
- Modify: `block-utils/Cargo.toml` (add `base64`)
- Modify: `block-utils/src/lib.rs`
- Test: `block-utils/src/lib.rs` `mod helper_tests`

**Interfaces:**
- Consumes: `Source`, `AssetKind`, `fetch_from_url`, `load_from_attachment`, `FfmpegReq`, `FfmpegResp`, `dispatch_ffmpeg_runtime`, `Envelope`, `ForUi`, `SkillError` (existing).
- Produces:
  - `pub fn build_media_envelope(out_bytes: &[u8], mime: &str, filename: String, for_llm: String, max_out: usize) -> Result<Vec<u8>, SkillError>` — **pure** (base64 → `data:` URL → `Envelope` → bytes, with the output size cap). Native-testable.
  - `#[cfg(target_arch = "wasm32")] pub fn resolve_source(source: Source, kind: AssetKind, max_in: usize) -> Result<(Vec<u8>, String, String), SkillError>` — the `match`-on-`Source` every media tool repeats.
  - `#[cfg(target_arch = "wasm32")] pub fn dispatch_ffmpeg(argv: Vec<String>, in_name: String, in_bytes: Vec<u8>, out_name: String) -> Result<Vec<u8>, SkillError>` — build `FfmpegReq`, dispatch, check exit code, return output bytes.

- [ ] **Step 1: Add the `base64` dependency**

In `block-utils/Cargo.toml` `[dependencies]`, add (match the version the image tools already use — confirm with `grep -h '^base64' blocks/image-*/Cargo.toml | head -1`; use that exact version, e.g.):

```toml
base64 = "0.22"
```

- [ ] **Step 2: Write the failing test (pure helper)**

Add to `block-utils/src/lib.rs` `mod helper_tests`:

```rust
    #[test]
    fn build_media_envelope_emits_data_url_and_caps_size() {
        let bytes = b"\x89PNG\r\n\x1a\n";
        let out = build_media_envelope(bytes, "image/png", "cat-resized.png".into(), "resized cat".into(), 1024)
            .expect("under cap");
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["for_llm"], "resized cat");
        assert_eq!(v["for_ui"]["mime"], "image/png");
        assert_eq!(v["for_ui"]["filename"], "cat-resized.png");
        let data_url = v["for_ui"]["data_url"].as_str().unwrap();
        assert!(data_url.starts_with("data:image/png;base64,"));

        let err = build_media_envelope(bytes, "image/png", "x.png".into(), "x".into(), 4)
            .expect_err("over cap");
        assert!(matches!(err, SkillError::TooLarge { .. }));
    }
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --manifest-path block-utils/Cargo.toml helper_tests::build_media_envelope 2>&1 | tail -12`
Expected: FAIL — `cannot find function build_media_envelope`.

- [ ] **Step 4: Implement the media helpers**

Append to `block-utils/src/lib.rs` (module scope):

```rust
// ---------------------------------------------------------------------------
// Media helpers. `build_media_envelope` is pure (native-testable); the source
// resolver and ffmpeg dispatcher call host imports and are wasm-only.
// ---------------------------------------------------------------------------

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

/// Encode `out_bytes` (enforcing `max_out`) as a `data:` URL and wrap it in the
/// standard image/video `Envelope` (`for_llm` summary + `for_ui` data URL).
pub fn build_media_envelope(
    out_bytes: &[u8],
    mime: &str,
    filename: String,
    for_llm: String,
    max_out: usize,
) -> Result<Vec<u8>, SkillError> {
    if out_bytes.len() > max_out {
        return Err(SkillError::TooLarge {
            kind: "output",
            bytes: out_bytes.len(),
            cap: max_out,
        });
    }
    let data_url = format!("data:{mime};base64,{}", B64.encode(out_bytes));
    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: mime.to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

/// Resolve a `Source` to `(bytes, mime, filename)` — the `url` fetch vs `ref`
/// attachment branch every media tool repeats.
#[cfg(target_arch = "wasm32")]
pub fn resolve_source(
    source: Source,
    kind: AssetKind,
    max_in: usize,
) -> Result<(Vec<u8>, String, String), SkillError> {
    match source {
        Source::Url(u) => fetch_from_url(&u, kind, max_in),
        Source::Ref(id) => load_from_attachment(&id, kind, max_in),
    }
}

/// Run one ffmpeg-runtime call and return the output bytes, mapping a non-zero
/// exit to `SkillError::FfmpegExitNonZero` (200-char log snippet).
#[cfg(target_arch = "wasm32")]
pub fn dispatch_ffmpeg(
    argv: Vec<String>,
    in_name: String,
    in_bytes: Vec<u8>,
    out_name: String,
) -> Result<Vec<u8>, SkillError> {
    let req = FfmpegReq {
        args: argv,
        inputs: vec![(in_name, in_bytes)],
        output: out_name,
    };
    let req_body = serde_json::to_vec(&req)
        .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let resp_bytes = dispatch_ffmpeg_runtime(&req_body)?;
    let ff: FfmpegResp = serde_json::from_slice(&resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;
    if ff.exit_code != 0 {
        return Err(SkillError::FfmpegExitNonZero {
            exit: ff.exit_code,
            snippet: ff.log.chars().take(200).collect(),
        });
    }
    Ok(ff.output)
}
```

- [ ] **Step 5: Run the pure test to verify pass + build for wasm**

Run: `cargo test --manifest-path block-utils/Cargo.toml helper_tests::build_media_envelope 2>&1 | tail -10`
Expected: `test result: ok. 1 passed`.
Run (verify the wasm-gated helpers compile for the real target):
`cargo build --manifest-path block-utils/Cargo.toml --target wasm32-unknown-unknown 2>&1 | tail -8`
Expected: `Finished` (no errors). (If `wasm32-unknown-unknown` is missing: `rustup target add wasm32-unknown-unknown`.)

- [ ] **Step 6: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add block-utils/Cargo.toml block-utils/Cargo.lock block-utils/src/lib.rs
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(block-utils): media helpers (build_media_envelope/resolve_source/dispatch_ffmpeg)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Shared `ArgvPlan` struct (and dropping `gizza_web_export!`)

**Decision (evidence-based):** the spec floated a `gizza_web_export!` macro, but the
current web export is **tool-specifically typed** —
`blocks/image-resize/web/src/lib.rs` exports
`build_argv(width: f64, height: f64, fit: &str, in_name: &str) -> Result<JsValue, JsValue>`.
Each ffmpeg tool's fields differ, so a generic `macro_rules!` can't capture the
signature without the caller re-stating it (no saving), and `wasm-bindgen`
requires concrete typed params (not a generic `JsValue` map). So **no macro.** The
one genuinely shared piece is the per-tool `#[derive(Serialize)] struct Plan { argv, out_name }`
duplicated in every ffmpeg web crate — promote it to `block-utils::ArgvPlan`. Web
wrappers keep their ~10-line typed body (it forwards to `core`; that's the right
amount of explicitness).

**Files:**
- Modify: `block-utils/src/lib.rs`
- Test: `block-utils/src/lib.rs` `mod helper_tests`

**Interfaces:**
- Produces: `pub struct ArgvPlan { pub argv: Vec<String>, pub out_name: String }` (`Serialize`). Consumed (Plan 4) by each ffmpeg tool's `web/src/lib.rs`, which builds it from `core` and returns `serde_wasm_bindgen::to_value(&plan)` — replacing the per-tool `struct Plan`.

- [ ] **Step 1: Write the failing test**

Add to `block-utils/src/lib.rs` `mod helper_tests`:

```rust
    #[test]
    fn argv_plan_serializes_to_argv_and_out_name() {
        let plan = ArgvPlan {
            argv: vec!["-i".into(), "in.png".into()],
            out_name: "out.png".into(),
        };
        let v = serde_json::to_value(&plan).unwrap();
        assert_eq!(v["argv"], serde_json::json!(["-i", "in.png"]));
        assert_eq!(v["out_name"], "out.png");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --manifest-path block-utils/Cargo.toml helper_tests::argv_plan 2>&1 | tail -12`
Expected: FAIL — `cannot find type ArgvPlan`.

- [ ] **Step 3: Implement `ArgvPlan`**

Append to `block-utils/src/lib.rs` (module scope):

```rust
/// The result an ffmpeg page tool's `build_argv` returns to the JS page driver:
/// the ffmpeg argument vector plus the output filename. Shared so every web
/// wrapper stops redefining an identical local `struct Plan`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArgvPlan {
    pub argv: Vec<String>,
    pub out_name: String,
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --manifest-path block-utils/Cargo.toml helper_tests::argv_plan 2>&1 | tail -10`
Expected: `test result: ok. 1 passed`.

- [ ] **Step 5: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add block-utils/src/lib.rs
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(block-utils): shared ArgvPlan struct for ffmpeg page exports

Drops the speculative gizza_web_export! macro (web exports are per-tool typed;
a generic macro can't capture them) in favor of sharing the one duplicated
piece — the { argv, out_name } plan struct.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Final verification

- [ ] Run the whole crate suite: `cargo test --manifest-path block-utils/Cargo.toml 2>&1 | tail -15` → all pass.
- [ ] Build for wasm: `cargo build --manifest-path block-utils/Cargo.toml --target wasm32-unknown-unknown 2>&1 | tail -5` → `Finished`.
- [ ] Sanity: existing tools still compile against the unchanged public items — `cargo build --manifest-path blocks/image-resize/Cargo.toml --target wasm32-unknown-unknown 2>&1 | tail -5` → `Finished` (no behavior of existing exports changed; only additions).

**Done when:** the foundation is in `block-utils` with green tests, wasm build clean, and no existing tool broken. **Handoff:** Plan 3 (generator + page runtime + docs) consumes `descriptor.json` (the serde from Task 1) and `to_schema_json`/`to_page_inputs` — note: `to_page_inputs()` is **generator-side** (Plan 3) reading the serialized descriptor, so it is *not* in this plan. Plan 4 (retrofit) wires `core::descriptor()`/`core::schema_json()` (Plan 1's expr `parameters`) and replaces each tool's `handle`/orchestration with these helpers (and `ArgvPlan` in the web crates) — that retrofit is what behavior-verifies `resolve_source`/`dispatch_ffmpeg`.
