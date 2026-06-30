/* tslint:disable */
/* eslint-disable */

/**
 * Build a CSS gradient declaration.
 *
 * - `colors`: comma/newline-separated CSS color stops.
 * - `gradient_type`: `"linear"` (blank → linear) / `"radial"` / `"conic"`.
 * - `angle`: degrees; blank/unparseable → NaN, which the core maps to the
 *   per-type default (180 linear, 0 conic).
 * - `shape`: `"ellipse"` (blank → ellipse) / `"circle"` (radial only).
 * - `repeating`: `"true"`/`"1"`/`"yes"`/`"on"` → repeating gradient.
 * - `interpolation`: color-interpolation space (blank → none), e.g. `oklch`.
 *
 * Throws a JS error string on invalid input.
 */
export function run(colors: string, gradient_type: string, angle: string, shape: string, repeating: string, interpolation: string): string;

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
