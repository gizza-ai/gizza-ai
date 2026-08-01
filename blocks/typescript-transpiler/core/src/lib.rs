//! typescript-transpiler core — a dependency-free, best-effort TypeScript → JavaScript
//! transpiler (type stripper).
//!
//! This is NOT the full TypeScript compiler. It is a deterministic, token-based
//! transformer that erases TypeScript-only *syntax* so the result runs as plain
//! JavaScript — the same idea as Node's `--experimental-strip-types` /
//! `--experimental-transform-types` and `@babel/preset-typescript`: it never
//! type-checks, it only rewrites syntax.
//!
//! What it does (see the module tests and the page copy for the full list):
//!   * removes type annotations (`: Type`) on variables, params, properties and
//!     return positions;
//!   * removes `interface` and `type` alias declarations;
//!   * removes `as` / `satisfies` assertions and postfix non-null `!`;
//!   * strips access modifiers (`public`/`private`/`protected`/`readonly`/
//!     `abstract`/`override`/`declare`) and the `?`/`!` on optional/definite
//!     members and params;
//!   * expands constructor *parameter properties* (`constructor(private x: T)` →
//!     an assignment `this.x = x` in the body);
//!   * removes generic type parameters/arguments in declarations and call sites
//!     (`foo<T>()` → `foo()`);
//!   * drops `implements` clauses and type-only imports/exports
//!     (`import type …`, `import { type X, Y }` → `import { Y }`);
//!   * either compiles `enum` to a runtime object (the default, matching tsc's
//!     output) or removes it (`enum_style = "strip"`);
//!   * optionally removes comments.
//!
//! It works on a token stream with balanced-bracket tracking rather than a full
//! parser, so it is fast and dependency-free, but it is best-effort: constructs
//! it does not model (namespaces/`module`, `const enum` inlining, decorator
//! emit, and downleveling to an older JS target) are documented as limitations
//! and passed through unchanged.

mod transform;

pub use transform::{parse_enum_style, transpile, EnumStyle, Options};

/// Convenience entry used by the chat/CLI block and the web wrapper: transpile
/// `input` with the default options (compile enums to objects, keep comments).
pub fn run(input: &str) -> Result<String, String> {
    transpile(input, &Options::default())
}
