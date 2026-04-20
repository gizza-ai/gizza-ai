// ai-bridge.js — WebLLM integration for local model inference
// Runs in the main page (NOT the Service Worker) — needs WebGPU access.
// Uses WebLLM via CDN import.

import { CreateMLCEngine } from 'https://cdn.jsdelivr.net/npm/@mlc-ai/web-llm@0.2.74/+esm';

let engine = null;
let currentModel = null;
let loadProgress = null;

// Models are listed in two tiers:
// - q4f32: works on all WebGPU GPUs (no shader-f16 needed)
// - q4f16: smaller/faster but needs shader-f16 extension
const AVAILABLE_MODELS = [
    // f32 models — broad compatibility
    { id: "SmolLM2-1.7B-Instruct-q4f32_1-MLC", name: "SmolLM2 1.7B (1.1GB)", size_mb: 1100, requires_f16: false },
    { id: "Qwen2.5-1.5B-Instruct-q4f32_1-MLC", name: "Qwen 2.5 1.5B (1.2GB)", size_mb: 1200, requires_f16: false },
    { id: "gemma-2-2b-it-q4f32_1-MLC", name: "Gemma 2 2B (1.7GB)", size_mb: 1700, requires_f16: false },
    { id: "Phi-3.5-mini-instruct-q4f32_1-MLC", name: "Phi 3.5 Mini (2.6GB)", size_mb: 2600, requires_f16: false },
    { id: "Llama-3.2-3B-Instruct-q4f32_1-MLC", name: "Llama 3.2 3B (2GB)", size_mb: 2000, requires_f16: false },
    // f16 models — need shader-f16 support
    { id: "SmolLM2-1.7B-Instruct-q4f16_1-MLC", name: "SmolLM2 1.7B f16 (1GB)", size_mb: 1000, requires_f16: true },
    { id: "Qwen2.5-1.5B-Instruct-q4f16_1-MLC", name: "Qwen 2.5 1.5B f16 (1GB)", size_mb: 1000, requires_f16: true },
];

export async function getAvailableModels() {
    // Filter models based on GPU capabilities
    let hasF16 = false;
    try {
        if (navigator.gpu) {
            const adapter = await navigator.gpu.requestAdapter();
            if (adapter) hasF16 = adapter.features.has('shader-f16');
        }
    } catch (e) { /* ignore */ }
    return AVAILABLE_MODELS.filter(m => !m.requires_f16 || hasF16);
}

export function getStatus() {
    return {
        available: typeof navigator !== 'undefined' && 'gpu' in navigator,
        loaded_model: currentModel,
        load_progress: loadProgress,
        webgpu_supported: typeof navigator !== 'undefined' && 'gpu' in navigator,
    };
}

export async function loadModel(modelId, onProgress) {
    if (engine && currentModel === modelId) return; // Already loaded

    // Unload previous
    if (engine) {
        await engine.unload();
        engine = null;
        currentModel = null;
    }

    loadProgress = { model: modelId, progress: 0, text: 'Starting...' };

    engine = await CreateMLCEngine(modelId, {
        initProgressCallback: (report) => {
            loadProgress = { model: modelId, progress: report.progress, text: report.text };
            if (onProgress) onProgress(loadProgress);
        }
    });

    currentModel = modelId;
    loadProgress = { model: modelId, progress: 1, text: 'Ready' };
}

export async function unloadModel() {
    if (engine) {
        await engine.unload();
        engine = null;
        currentModel = null;
        loadProgress = null;
    }
}

export async function chat(messages, onChunk) {
    if (!engine) throw new Error('No model loaded');

    const response = await engine.chat.completions.create({
        messages: messages,
        stream: true,
    });

    let fullContent = '';
    for await (const chunk of response) {
        const delta = chunk.choices[0]?.delta?.content || '';
        fullContent += delta;
        if (onChunk) onChunk(delta, fullContent);
    }

    return { content: fullContent, model: currentModel };
}

// Make available globally for the chat page JS
window.gizzaAI = { getAvailableModels, getStatus, loadModel, unloadModel, chat };

// Test hook — lets ad-hoc harnesses inject a fake engine. Not for production.
export function _testSetEngine(e) { engine = e; currentModel = 'test'; }

/**
 * Stream chat completion with tool-call support.
 *
 * @param {Array} messages OpenAI-format messages.
 * @param {Array} tools    OpenAI-format tools array (may be empty). When
 *                         empty, no tools field is sent to WebLLM.
 * @param {string} modelId WebLLM model id; engine must already be loaded
 *                         and currentModel must match.
 * @yields {{type:'token',delta:string} |
 *          {type:'tool_call',id:string,name:string,arguments:string} |
 *          {type:'done',finishReason:string}}
 */
export async function* inferStream(messages, tools, modelId) {
    if (!engine) throw new Error('No model loaded');
    if (currentModel !== modelId) throw new Error(`Model mismatch: have ${currentModel}, asked for ${modelId}`);

    const completion = engine.chat.completions.create({
        messages,
        tools: (tools && tools.length) ? tools : undefined,
        stream: true,
    });

    const buffers = new Map(); // index -> { id, name, arguments }
    let finishReason = null;

    for await (const chunk of completion) {
        const choice = chunk.choices?.[0];
        if (!choice) continue;
        const delta = choice.delta || {};

        if (delta.content) {
            yield { type: 'token', delta: delta.content };
        }

        if (Array.isArray(delta.tool_calls)) {
            for (const tc of delta.tool_calls) {
                const idx = tc.index ?? 0;
                let buf = buffers.get(idx);
                if (!buf) { buf = { id: '', name: '', arguments: '' }; buffers.set(idx, buf); }
                if (tc.id) buf.id = tc.id;
                if (tc.function?.name) buf.name = tc.function.name;
                if (tc.function?.arguments) buf.arguments += tc.function.arguments;
            }
        }

        if (choice.finish_reason) {
            finishReason = choice.finish_reason;
        }
    }

    const indices = [...buffers.keys()].sort((a, b) => a - b);
    for (const idx of indices) {
        const buf = buffers.get(idx);
        yield { type: 'tool_call', id: buf.id, name: buf.name, arguments: buf.arguments };
    }
    yield { type: 'done', finishReason: finishReason ?? 'stop' };
}

window.gizzaAI.inferStream = inferStream;

// ---------------------------------------------------------------------------
// External asset loader registry
// ---------------------------------------------------------------------------
// Plan A ships an empty registry. Plan C registers concrete loaders
// (ffmpeg.wasm, etc) via registerLoader(). Each loader takes the fetched
// bytes + the asset's manifest and returns an opaque handle (kept in
// _loadedAssets so subsequent loads of the same id are idempotent).

const _loaderRegistry = new Map(); // loader name -> async (bytes, manifest) => handle
const _loadedAssets = new Map();   // assetId -> handle

export function registerLoader(name, fn) {
    _loaderRegistry.set(name, fn);
}

async function _loadAssetInternal(manifest) {
    if (_loadedAssets.has(manifest.id)) return _loadedAssets.get(manifest.id);

    const loader = _loaderRegistry.get(manifest.loader);
    if (!loader) throw new Error(`unknown loader: ${manifest.loader}`);

    const resp = await fetch(manifest.url);
    if (!resp.ok) throw new Error(`fetch ${manifest.url}: ${resp.status}`);
    const bytes = new Uint8Array(await resp.arrayBuffer());

    if (manifest.sha256) {
        const digest = await crypto.subtle.digest('SHA-256', bytes);
        const hex = Array.from(new Uint8Array(digest))
            .map((b) => b.toString(16).padStart(2, '0'))
            .join('');
        if (hex !== manifest.sha256) {
            throw new Error(`sha256 mismatch for ${manifest.id}: expected ${manifest.sha256}, got ${hex}`);
        }
    }

    const handle = await loader(bytes, manifest);
    _loadedAssets.set(manifest.id, handle);
    return handle;
}

// Expose for chat page JS / tests that want to seed the registry without
// importing this module.
window.gizzaAI.registerLoader = registerLoader;

// ---------------------------------------------------------------------------
// Service Worker message bridge
// ---------------------------------------------------------------------------
// The SW forwards /b/local-llm/api/* requests + external-asset load
// requests here via postMessage. We call the appropriate function and
// send the result back. Both message types share the single SW listener.

if ('serviceWorker' in navigator) {
    navigator.serviceWorker.addEventListener('message', async (event) => {
        const msg = event.data;
        if (!msg) return;

        if (msg.type === 'load-asset-request') {
            try {
                await _loadAssetInternal(msg.manifest);
                navigator.serviceWorker.controller?.postMessage({
                    type: 'load-asset-response',
                    id: msg.id,
                    ok: true,
                });
            } catch (e) {
                navigator.serviceWorker.controller?.postMessage({
                    type: 'load-asset-response',
                    id: msg.id,
                    ok: false,
                    error: String(e?.message ?? e),
                });
            }
            return;
        }

        if (msg.type !== 'local-llm-request') return;

        const { id, action, body } = msg;

        // Send response back to SW
        function reply(data) {
            navigator.serviceWorker.controller?.postMessage({
                type: 'local-llm-response',
                id,
                data,
            });
        }
        function replyError(message, status = 500) {
            navigator.serviceWorker.controller?.postMessage({
                type: 'local-llm-response',
                id,
                error: message,
                status,
            });
        }

        try {
            switch (action) {
                case 'status':
                    reply(getStatus());
                    break;

                case 'models':
                    reply({ models: await getAvailableModels() });
                    break;

                case 'load': {
                    if (!body?.model_id) {
                        replyError('Missing model_id', 400);
                        break;
                    }
                    await loadModel(body.model_id);
                    reply({ loaded: body.model_id, status: 'ready' });
                    break;
                }

                case 'unload':
                    await unloadModel();
                    reply({ status: 'unloaded' });
                    break;

                case 'chat': {
                    if (!body?.messages) {
                        replyError('Missing messages', 400);
                        break;
                    }
                    const result = await chat(body.messages);
                    reply(result);
                    break;
                }

                case 'chat_stream': {
                    if (!body?.messages) { replyError('Missing messages', 400); break; }
                    try {
                        for await (const evt of inferStream(body.messages, body.tools || [], currentModel)) {
                            navigator.serviceWorker.controller?.postMessage({
                                type: 'local-llm-stream-event',
                                id,
                                event: evt,
                            });
                        }
                    } catch (e) {
                        replyError(String(e?.message ?? e));
                    }
                    break;
                }

                default:
                    replyError(`Unknown action: ${action}`, 404);
            }
        } catch (error) {
            replyError(String(error));
        }
    });
}

// ---------------------------------------------------------------------------
// Rust wasm-bindgen bridge
// ---------------------------------------------------------------------------

/**
 * Invoke the Service Worker's local-llm chat_stream handler and collect the
 * SSE response into a Uint8Array. Called from the agent block via
 * wasm-bindgen (see src/blocks/agent.rs).
 *
 * The SW must have set globalThis.__gizzaHandleLocalLlm before this is called.
 *
 * @param {string} bodyJson - OpenAI-format chat request body as JSON string.
 * @returns {Promise<Uint8Array>}
 */
export async function localLlmChatStream(bodyJson) {
    if (typeof globalThis.__gizzaHandleLocalLlm !== 'function') {
        throw new Error('__gizzaHandleLocalLlm not available — sw.js must set it');
    }
    const request = new Request('/b/local-llm/api/chat_stream', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: bodyJson,
    });
    const response = await globalThis.__gizzaHandleLocalLlm(request);
    if (!response.ok) {
        const text = await response.text();
        throw new Error(`chat_stream HTTP ${response.status}: ${text}`);
    }
    const buf = await response.arrayBuffer();
    return new Uint8Array(buf);
}
