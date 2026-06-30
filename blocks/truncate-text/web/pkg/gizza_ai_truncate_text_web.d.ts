/* tslint:disable */
/* eslint-disable */

/**
 * Shorten `text` to `length` characters or words, appending `ellipsis`.
 *
 * The standalone tool page passes every field value as a string:
 * - `length`: a unit count (blank/unparseable → 100; the core clamps the range).
 * - `unit`: "characters" or "words" (the page renders a `<select>`).
 * - `count_ellipsis` / `break_words`: `"true"`/`"1"`/`"on"`/`"yes"` → on; anything
 *   else (including blank) → off. The page renders these as checkboxes whose
 *   default-checked state comes from the descriptor.
 *
 * Throws a JS error string when `length` is out of range or `unit` is invalid.
 */
export function run(text: string, length: string, unit: string, ellipsis: string, count_ellipsis: string, break_words: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly run: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => [number, number, number, number];
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
