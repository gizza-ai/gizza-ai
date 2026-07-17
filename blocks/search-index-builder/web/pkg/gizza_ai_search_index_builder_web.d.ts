/* tslint:disable */
/* eslint-disable */

/**
 * Build a serialized inverted-index JSON from a pasted JSON array of documents.
 *
 * The standalone tool page passes every field value as a string, so the
 * boolean/integer params arrive as strings and are parsed here:
 * - `lowercase`/`remove_stopwords`/`pretty`: `"true"`/`"1"`/`"yes"`/`"on"` → on.
 *   `lowercase` defaults ON when blank (matches the schema default `true`);
 *   the other two default OFF when blank.
 * - `min_length`: a count `1`–20 (blank/unparseable → 1; the core clamps 1..=20).
 *
 * Throws a JS error string when `documents` is not a JSON array of objects,
 * on a duplicate ref, or on a malformed boost spec.
 */
export function run(documents: string, fields: string, id_field: string, store_fields: string, boosts: string, lowercase: string, remove_stopwords: string, min_length: string, pretty: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly run: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number) => [number, number, number, number];
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
