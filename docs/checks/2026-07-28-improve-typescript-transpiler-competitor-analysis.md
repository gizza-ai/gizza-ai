# typescript-transpiler competitor analysis (2026-07-28)

## Tool goal

Transpile TypeScript source to JavaScript by stripping type-only syntax locally in the gizza pure/WebAssembly model.

## Competitor/functionality scan

Sources reviewed from search results and known table-stakes behavior:

1. Official TypeScript compiler / Playground (`tsc`)
   - Table stakes: parses the full TypeScript grammar, optional type checking, project config, module resolution, JSX, decorators, enum emit, const enum behavior, declaration emit, sourcemaps, target/module options.
   - UX controls: input editor, target/module selectors, strictness/project options, diagnostics, output pane.
   - Fit: full parser/type checker/project model is out-of-model for this small pure block; simple enum emit and type stripping are in-model.

2. Babel TypeScript preset / online Babel REPL
   - Table stakes: syntax transform without type checking, strips annotations/interfaces/types/assertions, supports TSX with plugin settings, keeps modern JS transforms controlled by presets/plugins.
   - UX controls: preset/plugin toggles, source/output panes, examples.
   - Fit: syntax stripping is in-model; arbitrary Babel plugin ecosystem and JSX/downlevel transforms are out-of-model.

3. esbuild/SWC/Sucrase-style fast transpilers and online snippets
   - Table stakes: very fast TS syntax lowering, options for target, JSX, sourcemaps, minification, enum treatment, comments, and module format.
   - UX controls: target dropdowns, minify/comment toggles, copy output, example chips.
   - Fit: deterministic type stripping, comment removal, enum compile/strip choice, examples, and local execution are in-model; minification, sourcemaps, module bundling, JSX and target downleveling are out-of-model.

## Implemented in-model features

- Strip `interface` and `type` alias declarations.
- Strip variable/parameter/property/return type annotations for common snippet shapes.
- Strip `as` and `satisfies` assertions.
- Strip optional/definite markers and class access modifiers.
- Drop `implements` clauses and type-only imports/exports.
- Compile simple numeric enums to JavaScript objects, with a `strip` option.
- Optional comment removal.
- Error for empty input and 1 MB cap.
- Page controls: multiline input, enum handling select, remove-comments checkbox, example chips.

## Explicitly out of model / documented limits

- Full TypeScript parser and type checker.
- `tsconfig` project handling, module resolution, declaration files, diagnostics.
- JSX/TSX transforms.
- Namespace/module lowering and decorator metadata emit.
- `const enum` inlining and exact `tsc` enum reverse mappings.
- Sourcemaps, minification, bundling, target/module downleveling.

## Verification intent

The tests should assert exact output for representative snippets, enum compile/strip behavior, CLI exact output, generated page rendering, query-string deep links, and hygiene/schema drift checks.
