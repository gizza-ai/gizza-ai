/* tslint:disable */
/* eslint-disable */

/**
 * Parse raw `logs` into a structured table / JSON / CSV.
 *
 * The standalone tool page passes every field value as a string, so the
 * boolean/integer params arrive as strings and are parsed here:
 * - `format`: `auto` (blank) | `json` | `logfmt` | `syslog` | `common` | `combined`.
 * - `output`: `table` (blank) | `json` | `csv`.
 * - `level`:  `all` (blank) | `error` | `warn` | `info` | `debug` | `trace`.
 * - `filter`: text to keep matching lines (substring, or regex when `regex` is on).
 * - `regex`:  `"true"`/`"1"`/`"yes"`/`"on"` → treat `filter` as a regex; else off.
 * - `limit`:  a count 1–5000 (blank/unparseable → 0 → the core default of 200).
 *
 * Throws a JS error string on an invalid `format`/`output`/`level`, an invalid
 * regex, or undetectable auto input.
 */
export function run(logs: string, format: string, output: string, level: string, filter: string, regex: string, limit: string): string;

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
