/* tslint:disable */
/* eslint-disable */

/**
 * `strength` is 0–100 (0 = unchanged image; the page prefills the descriptor
 * default 40 and a CLEARED field arrives as 0). `mode` is `darken|lighten`
 * (empty defaults to darken). `color` is a name or hex (empty defaults to
 * black — the classic vignette; non-black tints require darken mode).
 * `center_x`/`center_y` are percent of the image size (50 = middle; a cleared
 * field arrives as 0 = the left/top edge). `format` is `keep|png|jpg|webp`
 * (empty defaults to keep). Returns `{ argv: string[], out_name }` or throws
 * a JS error string.
 */
export function build_argv(strength: number, mode: string, color: string, center_x: number, center_y: number, format: string, in_name: string): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly build_argv: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => [number, number, number];
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
