# Shared Tool Abstraction — Plan 1: `wafer_block` expression `parameters` (wafer-run)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `#[wafer_block(skill(parameters = …))]` accept an *expression* that evaluates to a JSON-schema string (e.g. `parameters = my_core::schema_json()`), not only a string literal — so gizza tools can derive their chat schema from a single-source descriptor.

**Architecture:** One change to the `wafer-block-macro` proc-macro. `parameters` keeps the string-literal form (compile-time JSON validation, fully backward-compatible) and additionally accepts any expression (validated at runtime in the generated `block_info()` / by the consumer's test). The generated `SkillTool.parameters` is built by coercing the value to `&str` via `AsRef` and `serde_json::from_str`.

**Tech Stack:** Rust proc-macro (`syn` 2, `quote` 1, `proc-macro2` 1), `serde_json`, `trybuild` for compile-fail tests.

**Spec:** `gizza-ai/docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md` §3 ("2a").

## Global Constraints

- **Repo:** `wafer-run` (this plan is the producer step; merge to wafer-run `main`, then fast-forward the local `/workspace/wafer-run` tree, before any gizza consumer plan — and only when no gizza build is in flight: shared-tree hazard).
- **Backward compatible:** the existing string-literal form must keep working unchanged, including its compile-time JSON validation (do not regress existing skill blocks).
- **wafer-run rules** (CLAUDE.md): fix the real issue; no magic/implicit mapping; no sync bridges.
- **CI parity:** CI clippy is 1.96 and fmt is nightly — run `cargo +1.96.0 clippy -p wafer-block-macro --all-targets -- -D warnings` and `cargo +nightly fmt` before pushing (see memory `wafer-run-ci-clippy-1-96`, `wafer-run-nightly-fmt`).
- **No new deps:** `proc-macro2`, `quote`, `syn`, `serde_json` are already dependencies of `wafer-block-macro`.

---

### Task 1: Accept an expression for `skill(parameters = …)`

**Files:**
- Modify: `wafer-run/crates/wafer-block-macro/src/lib.rs`
  - `struct SkillArgs` (≈ lines 254–258)
  - `fn parse_skill` (≈ lines 261–339)
  - `skill_tool_expr` emission (≈ lines 758–771)
- Test: `wafer-run/crates/wafer-block-macro/tests/skill_macro.rs` (append a module + test)

**Interfaces:**
- Consumes: nothing new.
- Produces: the macro now accepts `skill(parameters = <expr>)` where `<expr>: AsRef<str>` yielding JSON. Consumed by Plan 2 (`block-utils`) / the retrofit, where tools write `parameters = <core>::schema_json()`. The literal form `parameters = "…"` is unchanged.

- [ ] **Step 1: Write the failing test**

Append to `wafer-run/crates/wafer-block-macro/tests/skill_macro.rs` (after the existing `non_skill_block` module, before or after the `#[test]` fns — module order is irrelevant):

```rust
// A skill block whose `parameters` is an EXPRESSION (a `const &str`), not a
// string literal — the single-source-descriptor pattern gizza uses. Also
// exercises a `fn -> String` to prove the `AsRef<str>` coercion covers both.
mod skill_block_expr {
    use super::*;

    pub const SCHEMA: &str = r#"{
        "type": "object",
        "properties": { "x": { "type": "integer" } },
        "required": ["x"]
    }"#;

    pub fn schema_owned() -> String {
        SCHEMA.to_string()
    }

    pub struct ConstExprSkill;

    #[wafer_block(
        name = "test/const-expr-skill",
        version = "0.1.0",
        interface = "handler@v1",
        summary = "Schema via const expression",
        skill(
            description = "Schema supplied as a const &str expression.",
            parameters = SCHEMA
        )
    )]
    impl ConstExprSkill {
        fn handle(_msg: Message, _body: Vec<u8>) -> GuestResult {
            GuestResult::respond(b"{}".to_vec())
        }
    }
    impl ConstExprSkill {
        pub fn new() -> Self {
            Self
        }
    }
    #[wafer_async_trait]
    impl wafer_block::block::Block for ConstExprSkill {
        fn info(&self) -> wafer_block::types::BlockInfo {
            Self::block_info()
        }
        async fn handle(
            &self,
            _ctx: &dyn wafer_block::context::Context,
            _msg: wafer_block::core_types::Message,
            _input: wafer_block::streams::input::InputStream,
        ) -> wafer_block::streams::output::OutputStream {
            wafer_block::streams::output::OutputStream::drop_request()
        }
    }

    pub struct FnExprSkill;

    #[wafer_block(
        name = "test/fn-expr-skill",
        version = "0.1.0",
        interface = "handler@v1",
        summary = "Schema via fn-call expression",
        skill(
            description = "Schema supplied as a fn() -> String expression.",
            parameters = schema_owned()
        )
    )]
    impl FnExprSkill {
        fn handle(_msg: Message, _body: Vec<u8>) -> GuestResult {
            GuestResult::respond(b"{}".to_vec())
        }
    }
    impl FnExprSkill {
        pub fn new() -> Self {
            Self
        }
    }
    #[wafer_async_trait]
    impl wafer_block::block::Block for FnExprSkill {
        fn info(&self) -> wafer_block::types::BlockInfo {
            Self::block_info()
        }
        async fn handle(
            &self,
            _ctx: &dyn wafer_block::context::Context,
            _msg: wafer_block::core_types::Message,
            _input: wafer_block::streams::input::InputStream,
        ) -> wafer_block::streams::output::OutputStream {
            wafer_block::streams::output::OutputStream::drop_request()
        }
    }
}

#[test]
fn skill_parameters_accepts_const_expression() {
    let info = skill_block_expr::ConstExprSkill::block_info();
    let tool = info.tool.expect("skill(...) with const expr must set tool");
    assert_eq!(tool.description, "Schema supplied as a const &str expression.");
    assert_eq!(tool.parameters["type"], "object");
    assert_eq!(tool.parameters["properties"]["x"]["type"], "integer");
    assert_eq!(tool.parameters["required"], serde_json::json!(["x"]));
}

#[test]
fn skill_parameters_accepts_fn_call_expression() {
    let info = skill_block_expr::FnExprSkill::block_info();
    let tool = info.tool.expect("skill(...) with fn expr must set tool");
    assert_eq!(tool.parameters["properties"]["x"]["type"], "integer");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wafer-block-macro --test skill_macro 2>&1 | tail -20`
Expected: **compile error** at the new module — `#[wafer_block]: skill(parameters) value must be a string literal` (the macro currently rejects the non-literal `SCHEMA` / `schema_owned()`). This proves the test exercises the missing capability.

- [ ] **Step 3: Change `SkillArgs.parameters` to hold an expression**

In `wafer-run/crates/wafer-block-macro/src/lib.rs`, replace the struct (≈ lines 254–258):

```rust
/// Parsed skill(...) attribute — description and the JSON parameters schema for
/// the LLM-facing tool definition.
#[derive(Debug)]
struct SkillArgs {
    description: String,
    /// Tokens of an expression evaluating to the JSON-schema string. For the
    /// string-literal form this is the validated literal (JSON checked at
    /// macro-expansion time); for the expression form (e.g.
    /// `parameters = my_core::schema_json()`) these are the author's tokens,
    /// with JSON validity enforced at runtime in `block_info()` (and by the
    /// consumer's test) instead of at the macro site.
    parameters: proc_macro2::TokenStream,
}
```

- [ ] **Step 4: Rewrite `parse_skill` to accept a literal or an expression**

Replace the whole body of `fn parse_skill` (≈ lines 261–339) with:

```rust
fn parse_skill(meta: &syn::MetaList) -> syn::Result<SkillArgs> {
    let nested: Punctuated<syn::Meta, Token![,]> =
        meta.parse_args_with(Punctuated::parse_terminated)?;
    let mut description = None::<String>;
    let mut parameters = None::<proc_macro2::TokenStream>;
    for item in nested {
        let nv = match item {
            syn::Meta::NameValue(nv) => nv,
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "#[wafer_block]: unexpected token in skill(...)",
                ));
            }
        };
        let key = nv
            .path
            .get_ident()
            .ok_or_else(|| {
                syn::Error::new(
                    nv.path.span(),
                    "#[wafer_block]: expected identifier in skill(...)",
                )
            })?
            .to_string();
        match key.as_str() {
            "description" => {
                // description is always a string literal.
                let s = match &nv.value {
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) => s.value(),
                    other => {
                        return Err(syn::Error::new(
                            other.span(),
                            "#[wafer_block]: skill(description) value must be a string literal",
                        ));
                    }
                };
                description = Some(s);
            }
            "parameters" => {
                // Two accepted forms:
                //  1) a string literal — validate the JSON at macro-expansion
                //     time (author-controlled, fully known here) and emit the
                //     literal; preserves the original behavior exactly.
                //  2) any expression evaluating to a value `AsRef<str>` (e.g.
                //     `my_core::schema_json()`) — emit the tokens verbatim;
                //     JSON validity is enforced at runtime in `block_info()`.
                match &nv.value {
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) => {
                        let json = s.value();
                        if let Err(e) = serde_json::from_str::<serde_json::Value>(&json) {
                            return Err(syn::Error::new_spanned(
                                s,
                                format!(
                                    "#[wafer_block]: skill `parameters` is not valid JSON: {e}"
                                ),
                            ));
                        }
                        parameters = Some(quote! { #s });
                    }
                    expr => {
                        parameters = Some(quote! { #expr });
                    }
                }
            }
            other => {
                return Err(syn::Error::new(
                    nv.path.span(),
                    format!("#[wafer_block]: unknown skill attribute '{other}'"),
                ));
            }
        }
    }
    let description = description.ok_or_else(|| {
        syn::Error::new(
            meta.span(),
            "#[wafer_block]: skill(...) requires description = \"...\"",
        )
    })?;
    let parameters = parameters.ok_or_else(|| {
        syn::Error::new(
            meta.span(),
            "#[wafer_block]: skill(...) requires parameters = \"...\" or an expression",
        )
    })?;
    Ok(SkillArgs {
        description,
        parameters,
    })
}
```

- [ ] **Step 5: Update the emission to coerce the value to `&str`**

Replace the `skill_tool_expr` block (≈ lines 758–774):

```rust
    // Build the optional SkillTool expression for `block_info()`.
    let skill_tool_expr = if let Some(skill) = &skill_args {
        let description = &skill.description;
        let parameters_expr = &skill.parameters;
        quote! {
            info = info
                .tool(wafer_block::types::SkillTool {
                    description: #description.to_string(),
                    // `parameters` is either a string literal that `parse_skill`
                    // already validated as JSON, or an author-supplied expression
                    // evaluating to `&str`/`String`. Coerce to `&str` and parse;
                    // for the literal form this is infallible, for the expression
                    // form a malformed schema panics here (covered by a test).
                    parameters: serde_json::from_str(
                        ::core::convert::AsRef::<str>::as_ref(&(#parameters_expr))
                    )
                    .expect(concat!("skill parameters JSON parse error in block ", #name)),
                });
        }
    } else {
        quote! {}
    };
```

- [ ] **Step 6: Run the new tests to verify they pass**

Run: `cargo test -p wafer-block-macro --test skill_macro 2>&1 | tail -20`
Expected: PASS — including the existing `skill_attribute_sets_tool` / `no_skill_attribute_leaves_tool_unset` and the new `skill_parameters_accepts_const_expression` / `skill_parameters_accepts_fn_call_expression`.

- [ ] **Step 7: Run the full macro suite (incl. trybuild compile-fail) to verify no regressions**

Run: `cargo test -p wafer-block-macro 2>&1 | tail -30`
Expected: PASS. The trybuild fixtures (`fail_caps_and_skill`, `fail_invalid_block_name`, `fail_missing_new`) are unaffected — none assert the removed "parameters must be a string literal" error (verified). If trybuild reports `.stderr` drift unrelated to this change, regenerate with `TRYBUILD=overwrite cargo test -p wafer-block-macro` and inspect the diff before keeping it.

- [ ] **Step 8: Lint + format at CI parity**

Run: `cargo +1.96.0 clippy -p wafer-block-macro --all-targets -- -D warnings`
Expected: no warnings.
Run: `cargo +nightly fmt`
Expected: clean (no diff in this file after formatting; if fmt touches unrelated files, do not stage them).

- [ ] **Step 9: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/wafer-run add crates/wafer-block-macro/src/lib.rs crates/wafer-block-macro/tests/skill_macro.rs
git -C /home/joris/Programs/suppers-ai/workspace/wafer-run commit -m "feat(wafer-block-macro): accept an expression for skill(parameters)

Allow #[wafer_block(skill(parameters = <expr>))] where <expr>: AsRef<str>
yields a JSON schema string, alongside the existing string-literal form
(which keeps its compile-time JSON validation). Lets consumers derive the
chat schema from a single source. Validity for the expression form is
enforced at runtime in block_info() and by tests.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

**Done when:** all `wafer-block-macro` tests pass (incl. trybuild), clippy/fmt clean. **Handoff:** open a wafer-run PR, merge to `main`, then `git -C /workspace/wafer-run checkout main && git pull` to fast-forward the local tree gizza's `.cargo` patch points at — do this only when no gizza build is in flight (shared-tree hazard). Plan 2 (block-utils foundation) depends on this being on wafer-run `main` for gizza CI (which clones wafer-run `main`).
