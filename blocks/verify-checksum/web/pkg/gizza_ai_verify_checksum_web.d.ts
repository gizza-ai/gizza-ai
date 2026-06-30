/* tslint:disable */
/* eslint-disable */

/**
 * Verify that `text` matches the `expected` checksum.
 *
 * The standalone tool page passes every field value as a string:
 * - `text`: the input data to hash and check.
 * - `expected`: the expected checksum (hex, optionally `0x`-prefixed and any
 *   case, or standard base64).
 * - `algorithm`: `"auto"` (blank → auto) or an explicit id (`"sha256"`, …).
 * - `input_encoding`: `"text"` (blank → text) / `"hex"` / `"base64"`.
 *
 * Returns a multi-line report (MATCH/MISMATCH + algorithm + expected/actual
 * digests). Throws a JS error string on an invalid algorithm/encoding or an
 * undecodable input/expected value.
 */
export function run(text: string, expected: string, algorithm: string, input_encoding: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly run: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
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
