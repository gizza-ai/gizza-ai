/* tslint:disable */
/* eslint-disable */

/**
 * Parse, validate, merge and secret-mask a `.env` file.
 *
 * - `env`: the primary `.env` contents (required).
 * - `merge`: an optional overlay `.env` (its keys override).
 * - `required_keys`: comma-separated keys that must be present.
 * - `mask_secrets`: `"false"`/`"0"`/`"no"`/`"off"` turns masking OFF; blank or
 *   anything else keeps the default ON (the checkbox defaults to checked).
 * - `sort_keys`: `"true"`/`"1"`/`"yes"`/`"on"` sorts keys alphabetically.
 * - `output`: `report` | `normalized` | `example` | `json` (blank → report).
 *
 * Throws a JS error string on an invalid `output` mode.
 */
export function run(env: string, merge: string, required_keys: string, mask_secrets: string, sort_keys: string, output: string): string;

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
