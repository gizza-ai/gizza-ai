/* tslint:disable */
/* eslint-disable */

/**
 * Extract a Markdown section by heading.
 *
 * The standalone tool page passes every field value as a string, so the boolean
 * params arrive as strings and are parsed here:
 * - `markdown`: the full document.
 * - `heading`: the heading text to find.
 * - `match_mode`: `"exact"` / `"exact_case"` / `"contains"` (blank → exact).
 * - `include_subsections`: `"true"`/`"1"`/`"yes"`/`"on"` → keep nested
 *   subsections; the checkbox defaults to checked (true).
 * - `include_heading`: same truthy parsing; defaults to checked (true).
 *
 * Throws a JS error string on a blank heading, an invalid match mode, or no
 * matching heading.
 */
export function run(markdown: string, heading: string, match_mode: string, include_subsections: string, include_heading: string): string;

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
