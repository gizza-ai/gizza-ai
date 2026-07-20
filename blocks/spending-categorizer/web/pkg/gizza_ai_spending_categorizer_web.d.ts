/* tslint:disable */
/* eslint-disable */

/**
 * Auto-categorize a bank/credit-card CSV export and summarize spending.
 *
 * - `data`: the pasted CSV, with a header row.
 * - `description_column` / `amount_column` / `debit_column` / `credit_column`
 *   / `date_column`: column names (blank = auto-detect).
 * - `rules`: newline `keyword = Category` rules, checked before built-ins.
 * - `output`: `both` / `summary` / `csv`.
 * - `currency`: symbol (`$`, prefixed) or code (`USD`, suffixed).
 * - `delimiter`: `auto` / `comma` / `semicolon` / `tab` / `pipe`.
 * - `invert_amount`: `"true"`/`"1"` to flip the sign of every amount.
 *
 * Throws a JS error string on an unknown enum value or unparsable input.
 */
export function run(data: string, description_column: string, amount_column: string, debit_column: string, credit_column: string, date_column: string, rules: string, output: string, currency: string, delimiter: string, invert_amount: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly run: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number, v: number) => [number, number, number, number];
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
