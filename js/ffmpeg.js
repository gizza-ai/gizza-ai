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
        const mod = await import(
            `https://cdn.jsdelivr.net/npm/@ffmpeg/ffmpeg@${FFMPEG_VERSION}/+esm`
        );
        const inst = new mod.FFmpeg();
        await inst.load({
            coreURL: `https://cdn.jsdelivr.net/npm/@ffmpeg/core@${CORE_VERSION}/dist/umd/ffmpeg-core.js`,
            wasmURL: `https://cdn.jsdelivr.net/npm/@ffmpeg/core@${CORE_VERSION}/dist/umd/ffmpeg-core.wasm`,
        });
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
