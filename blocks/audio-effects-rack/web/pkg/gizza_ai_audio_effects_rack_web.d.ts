/* tslint:disable */
/* eslint-disable */

/**
 * `reverb` is `none|room|hall|plate`, `chorus` `none|light|deep`,
 * `compression` `none|light|medium|heavy` (empty selects default to their
 * first `none` value). `echo` is a delay in ms (0–1000; 0 = off) and `tremolo`
 * a rate in Hz (0 or 0.1–20; 0 = off) — empty fields arrive as 0. `format` is
 * `mp3|wav|ogg|flac|m4a` (empty defaults to mp3). Every stage off throws the
 * guiding no-op error. Returns `{ argv, out_name }` or throws an error string.
 */
export function build_argv(reverb: string, echo: number, chorus: string, tremolo: number, compression: string, format: string, in_name: string): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly build_argv: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => [number, number, number];
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
