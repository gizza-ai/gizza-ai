/* tslint:disable */
/* eslint-disable */

/**
 * Build a table of contents from a Markdown or HTML document.
 *
 * The standalone tool page passes every field value as a string:
 * - `document`: the Markdown or HTML source.
 * - `input_format`: `"auto"`/`"markdown"`/`"html"` (blank → auto-detect).
 * - `output_format`: `"markdown"`/`"html"` (blank → markdown).
 * - `min_level`/`max_level`: heading levels 1-6 (blank/unparseable → 1 / 6).
 * - `ordered`: `"true"`/`"1"`/`"on"`/`"yes"` → numbered list (default false).
 *
 * Throws a JS error string on empty input, an invalid format, or no headings.
 */
export function run(document: string, input_format: string, output_format: string, min_level: string, max_level: string, ordered: string): string;

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
