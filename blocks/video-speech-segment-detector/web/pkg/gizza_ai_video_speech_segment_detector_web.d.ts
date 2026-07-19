/* tslint:disable */
/* eslint-disable */

/**
 * Pass-1 plan. NaN numeric fields (empty inputs) fall back to the defaults;
 * field order matches page/meta.toml.
 */
export function detect_argv(threshold_db: number, min_silence: number, voice_band: string, in_name: string): any;

/**
 * Friendly message for a failed detect pass: the common no-audio-track case
 * gets a clear explanation; anything else surfaces the log's last line.
 */
export function error_message(log: string): string;

/**
 * Pass-2: parse the detect pass's ffmpeg log and render the report. Errors
 * (unreadable log, bad enum values) come back as strings the page shows
 * verbatim.
 */
export function segments_report(log: string, min_speech: number, pad: number, segments: string, output: string): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly detect_argv: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly error_message: (a: number, b: number) => [number, number];
    readonly segments_report: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly __wafer_alloc: (a: number) => number;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
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
