/* tslint:disable */
/* eslint-disable */

/**
 * Format a Unix timestamp (seconds, supplied by JS as `Date.now()/1000`) as UTC
 * RFC-3339, matching the chat skill's output exactly.
 *
 * The parameter is `f64` (not `i64`) on purpose: wasm-bindgen marshals an `i64`
 * argument as a JS `BigInt`, but the page driver passes a plain JS number, so an
 * `i64` here throws "Cannot convert … to a BigInt" at call time. `f64` accepts a
 * JS number directly; whole seconds are well within f64's exact-integer range.
 */
export function format_time(unix_secs: number): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly format_time: (a: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
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
