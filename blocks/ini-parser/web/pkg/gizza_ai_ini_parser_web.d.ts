/* tslint:disable */
/* eslint-disable */

/**
 * Parse INI/conf `ini` text into structured JSON.
 *
 * The standalone tool page passes every field value as a string, so the
 * boolean params arrive as strings and are parsed here:
 * - `output`: `json` (blank) | `flat` | `report`.
 * - `duplicate_keys`: `last` (blank) | `first` | `array` | `error`.
 * - `detect_types`: `"true"`/`"1"`/`"yes"`/`"on"` → coerce booleans/numbers; else off.
 * - `comments`: `both` (blank) | `semicolon` | `hash`.
 * - `inline_comments`: `"true"`/`"1"`/`"yes"`/`"on"` → strip trailing comments; else off.
 *
 * Throws a JS error string on an invalid `output`/`duplicate_keys`/`comments`,
 * a malformed line, or (with `duplicate_keys=error`) a duplicate key.
 */
export function run(ini: string, output: string, duplicate_keys: string, detect_types: string, comments: string, inline_comments: string): string;

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
