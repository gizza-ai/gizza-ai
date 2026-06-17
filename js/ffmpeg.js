//! gizza-ai ffmpeg JS bridge.
//!
//! Exports a single function `ffmpegExec(argsJson, inputsJson, outputName)`
//! that the Rust BrowserFfmpegService calls via wasm-bindgen.
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
 * Run ffmpeg with the given CLI args.
 *
 * @param {string} argsJson    JSON-encoded array of strings, e.g. '["-i","in","out"]'
 * @param {string} inputsJson  JSON-encoded array of {name, bytes_b64}
 * @param {string} outputName  Filename to read back from ffmpeg's virtual FS
 * @returns {Promise<{exit_code:number, output_b64:string, log:string}>}
 */
export async function ffmpegExec(argsJson, inputsJson, outputName) {
    const args = JSON.parse(argsJson);
    const inputs = JSON.parse(inputsJson);
    const ffmpeg = await ensureFfmpeg();

    let log = "";
    const onLog = ({ message }) => { log += message + "\n"; };
    ffmpeg.on("log", onLog);

    try {
        for (const { name, bytes_b64 } of inputs) {
            await ffmpeg.writeFile(name, b64ToUint8(bytes_b64));
        }
        const exit_code = await ffmpeg.exec(args);

        let output_b64 = "";
        try {
            const out = await ffmpeg.readFile(outputName);
            const u8 = out instanceof Uint8Array ? out : new TextEncoder().encode(out);
            output_b64 = uint8ToB64(u8);
        } catch (_) {
            // ffmpeg failed before producing output — leave output_b64 empty.
        }

        return { exit_code, output_b64, log };
    } finally {
        ffmpeg.off("log", onLog);
    }
}
