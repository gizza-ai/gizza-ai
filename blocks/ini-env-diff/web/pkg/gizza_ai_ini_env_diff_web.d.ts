/* tslint:disable */
/* eslint-disable */

/**
 * Diff two `.env`/`.ini` config files key-by-key.
 *
 * - `left`: the first (old/base) config file contents (required).
 * - `right`: the second (new/compared) config file contents (required).
 * - `format`: `auto` (default) | `env` | `ini` (blank → auto).
 * - `ignore_case`: `"true"`/`"1"`/`"yes"`/`"on"` compares keys case-insensitively
 *   (default-false checkbox — anything else stays case-sensitive).
 * - `mask_secrets`: `"true"`/`"1"`/`"yes"`/`"on"` masks sensitive-looking values
 *   (default-false checkbox).
 * - `output`: `report` (default) | `json` (blank → report).
 *
 * Throws a JS error string on an invalid `format` or `output` mode.
 */
export function run(left: string, right: string, format: string, ignore_case: string, mask_secrets: string, output: string): string;

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
