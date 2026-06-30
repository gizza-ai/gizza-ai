/* tslint:disable */
/* eslint-disable */

/**
 * Split `text` into a numbered tweet thread, returned as plain text with the
 * tweets separated by a blank line.
 *
 * The standalone tool page passes every field value as a string, so the
 * integer/boolean params arrive as strings and are parsed here:
 * - `limit`: max chars per tweet (blank/unparseable → 280; core clamps the range).
 * - `numbering`: `"parens"` (blank → parens) | `"slash"` | `"dotted"` | `"none"`.
 * - `count`: `"chars"` (blank → chars) or `"utf16"`.
 * - `prefer_sentences`: `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
 *   (The page renders this checkbox checked by default, so a normal load sends `"true"`.)
 *
 * Throws a JS error string on an invalid `numbering`/`count`, an out-of-range
 * `limit`, or empty input.
 */
export function run(text: string, limit: string, numbering: string, count: string, prefer_sentences: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly run: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number, number, number];
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
