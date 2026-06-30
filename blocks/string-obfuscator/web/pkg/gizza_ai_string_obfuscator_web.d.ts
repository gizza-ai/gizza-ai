/* tslint:disable */
/* eslint-disable */

/**
 * Mask or obfuscate `text`.
 *
 * The standalone tool page passes every field value as a string, so the
 * integer params arrive as strings and are parsed here (blank/unparseable →
 * the documented default).
 * - `mode`: `"mask"` (default) / `"rot"` / `"leetspeak"` / `"homoglyph"`.
 * - `mask_char`: replacement char for `mask` mode (blank → `*`).
 * - `keep_start` / `keep_end`: chars to keep visible in `mask` mode (→ 0).
 * - `rot_n`: ROT shift for `rot` mode (blank → 13).
 *
 * Throws a JS error string on an invalid `mode`.
 */
export function run(text: string, mode: string, mask_char: string, keep_start: string, keep_end: string, rot_n: string): string;

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
