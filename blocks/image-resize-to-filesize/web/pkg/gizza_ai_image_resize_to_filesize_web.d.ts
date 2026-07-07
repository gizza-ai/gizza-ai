/* tslint:disable */
/* eslint-disable */

/**
 * Defensive single-pass fallback (see module docs). Signature matches the page
 * field order (`target_kb`, `format`, `max_width`) so the shared driver can
 * still produce a valid image if `custom.js` doesn't load — it just encodes at
 * a fixed quality instead of searching the target.
 */
export function build_argv(_target_kb: number, format: string, max_width: number, in_name: string): any;

/**
 * Build the ffmpeg argv for ONE encode attempt at an explicit `quality`
 * (5-95). Called by `custom.js` once per binary-search step. Numeric params are
 * `f64` to avoid the wasm-bindgen BigInt path.
 */
export function build_attempt(format: string, quality: number, max_width: number, in_name: string): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly build_argv: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly build_attempt: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly __wafer_alloc: (a: number) => number;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
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
