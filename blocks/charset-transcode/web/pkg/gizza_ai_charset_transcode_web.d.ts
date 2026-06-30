/* tslint:disable */
/* eslint-disable */

/**
 * Re-decode `text` from the legacy charset `from` into clean UTF-8.
 *
 * The standalone tool page passes every field value as a string:
 * - `text`: the garbled input.
 * - `from`: the source charset label (e.g. `"windows-1252"`); `"auto"` or blank
 *   auto-detects.
 * - `errors`: `"replace"` (blank → replace) or `"strict"`.
 * - `passes`: a count `1`–8 (blank/unparseable → 1; the core clamps the range)
 *   for un-nesting double-encoded mojibake.
 *
 * Throws a JS error string on an unknown charset, a bad `errors` value, or when
 * the charset can't repair the input.
 */
export function run(text: string, from: string, errors: string, passes: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly run: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
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
