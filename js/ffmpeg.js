//! gizza-ai ffmpeg JS bridge.
//!
//! Exports `ffmpegExecBytes(args, inputs, outputName)` (raw bytes, used by the
//! tool pages and the editor) and its base64/JSON wrapper
//! `ffmpegExec(argsJson, inputsJson, outputName)`, which the Rust
//! BrowserFfmpegService calls via wasm-bindgen.
//!
//! @ffmpeg/ffmpeg + @ffmpeg/core are loaded lazily on first call from the
//! jsdelivr CDN. Subsequent calls reuse the same FFmpeg instance.

let ffmpegInstance = null;
let ffmpegInstancePromise = null;

const FFMPEG_VERSION = "0.12.15";
const CORE_VERSION = "0.12.10";

async function ensureFfmpeg() {
    if (ffmpegInstance) return ffmpegInstance;
    if (ffmpegInstancePromise) return ffmpegInstancePromise;

    ffmpegInstancePromise = (async () => {
        // The ESM build of @ffmpeg/ffmpeg creates a *module worker* via
        //   new Worker(new URL("./worker.js", import.meta.url), {type:"module"})
        // where import.meta.url is the jsDelivr CDN URL. Chrome blocks cross-origin
        // module workers unless the origin opts in to COOP/COEP. Work around this
        // by fetching the worker script, rewriting its relative imports to absolute
        // CDN URLs, and serving it from a same-origin blob URL. The blob worker is
        // then passed as classWorkerURL so the FFmpeg class uses it instead.
        const CDN_ESM_BASE = `https://cdn.jsdelivr.net/npm/@ffmpeg/ffmpeg@${FFMPEG_VERSION}/dist/esm/`;
        const WORKER_URL = CDN_ESM_BASE + "worker.js";

        const mod = await import(
            `https://cdn.jsdelivr.net/npm/@ffmpeg/ffmpeg@${FFMPEG_VERSION}/+esm`
        );

        let classWorkerURL;
        try {
            const resp = await fetch(WORKER_URL);
            if (resp.ok) {
                // Rewrite relative ./foo.js imports to absolute CDN URLs so they
                // resolve correctly when the script runs from a blob: origin.
                let src = await resp.text();
                src = src.replace(/from "(\.[^"]+)"/g, (_, rel) =>
                    `from "${new URL(rel, WORKER_URL).href}"`
                );
                src = src.replace(/import "(\.[^"]+)"/g, (_, rel) =>
                    `import "${new URL(rel, WORKER_URL).href}"`
                );
                classWorkerURL = URL.createObjectURL(
                    new Blob([src], { type: "text/javascript" })
                );
            }
        } catch (_) {
            // If fetch or blob creation fails, fall back to the default load
            // (will only work when COOP/COEP headers are served).
        }

        const inst = new mod.FFmpeg();
        // Use the ESM core when our blob worker imports it via dynamic import():
        // the UMD build doesn't expose a default export so .default would be
        // undefined, which ffmpeg's worker treats as a load failure.
        const loadOpts = {
            coreURL: `https://cdn.jsdelivr.net/npm/@ffmpeg/core@${CORE_VERSION}/dist/esm/ffmpeg-core.js`,
            wasmURL: `https://cdn.jsdelivr.net/npm/@ffmpeg/core@${CORE_VERSION}/dist/esm/ffmpeg-core.wasm`,
        };
        if (classWorkerURL) loadOpts.classWorkerURL = classWorkerURL;
        await inst.load(loadOpts);
        ffmpegInstance = inst;
        return inst;
    })();

    return ffmpegInstancePromise;
}

function b64ToUint8(b64) {
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
}

function uint8ToB64(arr) {
    let s = "";
    const chunk = 0x8000;
    for (let i = 0; i < arr.length; i += chunk) {
        s += String.fromCharCode.apply(null, arr.subarray(i, i + chunk));
    }
    return btoa(s);
}

/**
 * Run ffmpeg with the given CLI args on raw bytes (no base64).
 *
 * Calls are serialized FIFO: they share one FFmpeg instance and its virtual
 * FS, and tools reuse fixed in/out filenames — interleaved
 * writeFile/exec/readFile from overlapping calls would cross-wire inputs and
 * outputs (e.g. an earlier call reading back a later call's output).
 *
 * Input bytes are copied before they are handed to ffmpeg: its writeFile
 * transfers the buffer to the ffmpeg worker, which would otherwise detach the
 * caller's array.
 *
 * @param {string[]} args                               e.g. ["-i","in","out"]
 * @param {{name:string, bytes:Uint8Array}[]} inputs   virtual-FS input files
 * @param {string} outputName  Filename to read back from ffmpeg's virtual FS
 * @returns {Promise<{exit_code:number, output:Uint8Array|null, log:string}>}
 */
let execChain = Promise.resolve();

export function ffmpegExecBytes(args, inputs, outputName) {
    const run = execChain.then(() => execOnce(args, inputs, outputName));
    execChain = run.then(() => {}, () => {});
    return run;
}

/**
 * Base64/JSON form of {@link ffmpegExecBytes} — the contract the chat app's
 * Rust BrowserFfmpegService (via wasm-bindgen) and older page modules use.
 *
 * @param {string} argsJson    JSON-encoded array of strings
 * @param {string} inputsJson  JSON-encoded array of {name, bytes_b64}
 * @param {string} outputName  Filename to read back from ffmpeg's virtual FS
 * @returns {Promise<{exit_code:number, output_b64:string, log:string}>}
 */
export async function ffmpegExec(argsJson, inputsJson, outputName) {
    const inputs = JSON.parse(inputsJson).map(({ name, bytes_b64 }) => ({
        name,
        bytes: b64ToUint8(bytes_b64),
    }));
    const { exit_code, output, log } = await ffmpegExecBytes(JSON.parse(argsJson), inputs, outputName);
    return { exit_code, output_b64: output ? uint8ToB64(output) : "", log };
}

async function execOnce(args, inputs, outputName) {
    const ffmpeg = await ensureFfmpeg();

    let log = "";
    const onLog = ({ message }) => { log += message + "\n"; };
    ffmpeg.on("log", onLog);

    try {
        for (const { name, bytes } of inputs) {
            await ffmpeg.writeFile(name, bytes.slice());
        }
        const exit_code = await ffmpeg.exec(args);

        let output = null;
        try {
            const out = await ffmpeg.readFile(outputName);
            output = out instanceof Uint8Array ? out : new TextEncoder().encode(out);
        } catch (_) {
            // ffmpeg failed before producing output — leave output null.
        }

        return { exit_code, output, log };
    } finally {
        ffmpeg.off("log", onLog);
    }
}
