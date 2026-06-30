/* tslint:disable */
/* eslint-disable */

/**
 * Split `text` on a delimiter into one item per line.
 *
 * The standalone tool page passes every field value as a string, so the
 * boolean params arrive as strings and are parsed here:
 * - `delimiter`: the substring to split on (blank → the core's literal-mode
 *   empty-delimiter error; use mode whitespace/chars instead). Escapes \n \t \r \\.
 * - `mode`: `"literal"`/`"whitespace"`/`"chars"` (blank → literal).
 * - `trim` / `remove_empty`: `"true"`/`"1"`/`"yes"`/`"on"` → on; else off.
 *
 * Throws a JS error string on an invalid `mode` or an empty literal delimiter.
 */
export function run(text: string, delimiter: string, mode: string, trim: string, remove_empty: string): string;

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
