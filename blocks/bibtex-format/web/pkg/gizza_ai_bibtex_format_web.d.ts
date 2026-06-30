/* tslint:disable */
/* eslint-disable */

/**
 * Validate, sort, and pretty-print BibTeX source.
 *
 * The standalone tool page passes every field value as a string, so the
 * boolean/integer params arrive as strings and are parsed here:
 * - `sort`: `"none"`/`"key"`/`"type-key"` (blank → none).
 * - `sort_fields` / `align_values`: `"true"`/`"1"`/`"yes"`/`"on"` → on.
 * - `lowercase_type` / `check_duplicates`: same truthy set; the page passes
 *   "false" when unchecked and "true" when checked, so a default-checked box → on.
 * - `indent`: a count `0`–16 (blank/unparseable → 2; the core clamps the range).
 *
 * Throws a JS error string on a BibTeX parse error, a duplicate cite key, or an
 * invalid sort mode.
 */
export function run(bibtex: string, sort: string, sort_fields: string, indent: string, align_values: string, lowercase_type: string, check_duplicates: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly run: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => [number, number, number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
