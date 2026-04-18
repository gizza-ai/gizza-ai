// sw.js — Service Worker that runs gizza-ai via WASM
import init, { initialize, handle_request } from './gizza_ai.js';

let initialized = false;
let initPromise = null;

async function ensureInitialized() {
    if (initialized) return;
    if (initPromise) return await initPromise;
    initPromise = (async () => {
        console.log('[gizza-ai] Loading WASM module...');
        await init();
        console.log('[gizza-ai] Initializing runtime...');
        await initialize();
        initialized = true;
        console.log('[gizza-ai] Runtime ready.');
    })();
    await initPromise;
}

self.addEventListener('install', (event) => {
    console.log('[gizza-ai] Service Worker installing...');
    event.waitUntil(self.skipWaiting());
});

self.addEventListener('activate', (event) => {
    console.log('[gizza-ai] Service Worker activating...');
    event.waitUntil(self.clients.claim());
});

// ---------------------------------------------------------------------------
// Local-LLM postMessage bridge
// ---------------------------------------------------------------------------
// local-llm runs in the main thread (WebGPU needs main thread access).
// The SW forwards /b/local-llm/api/* requests to a page client via
// postMessage and waits for the response.

const pendingLocalLlm = new Map(); // requestId -> { resolve, reject }
const pendingLocalLlmStreams = new Map(); // requestId -> { controller, closed }

self.addEventListener('message', (event) => {
    const msg = event.data;
    if (!msg) return;

    if (msg.type === 'local-llm-stream-event') {
        const stream = pendingLocalLlmStreams.get(msg.id);
        if (!stream || stream.closed) return;
        const { type, ...data } = msg.event;
        const sseFrame = `event: ${type}\ndata: ${JSON.stringify(data)}\n\n`;
        stream.controller.enqueue(new TextEncoder().encode(sseFrame));
        if (type === 'done') {
            stream.closed = true;
            pendingLocalLlmStreams.delete(msg.id);
            stream.controller.close();
        }
        return;
    }

    if (msg.type === 'local-llm-response') {
        // If this id corresponds to a streaming request (error teardown path),
        // close the SSE stream instead of the one-shot resolver.
        const stream = pendingLocalLlmStreams.get(msg.id);
        if (stream && !stream.closed) {
            stream.closed = true;
            pendingLocalLlmStreams.delete(msg.id);
            if (msg.error) {
                const errFrame = `event: error\ndata: ${JSON.stringify({ error: msg.error, status: msg.status ?? 500 })}\n\n`;
                stream.controller.enqueue(new TextEncoder().encode(errFrame));
            }
            stream.controller.close();
            return;
        }
        // One-shot path:
        const pending = pendingLocalLlm.get(msg.id);
        if (pending) {
            pendingLocalLlm.delete(msg.id);
            pending.resolve(msg);
        }
        return;
    }

    // Asset loader bridge: route reply to ai-bridge.js's pending-load map.
    // ai-bridge.js exposes the resolver on globalThis because this script
    // (sw.js) doesn't import the wasm-bindgen-generated bridge module.
    if (msg.type === 'load-asset-response') {
        if (typeof globalThis.__gizzaCompleteAssetLoad === 'function') {
            globalThis.__gizzaCompleteAssetLoad(msg.id, {
                status: msg.ok ? 'ready' : 'failed',
                error: msg.ok ? undefined : msg.error,
            });
        }
        return;
    }
});

let llmRequestId = 0;

async function handleLocalLlm(request) {
    const url = new URL(request.url);
    const path = url.pathname;

    // Streaming endpoint — SSE response. Drive a separate stream-state map so
    // one-shot state doesn't collide.
    if (path === '/b/local-llm/api/chat_stream') {
        const clients = await self.clients.matchAll({ type: 'window', includeUncontrolled: false });
        if (clients.length === 0) {
            return new Response(
                JSON.stringify({ error: 'no_client', message: 'No active page — open the app to use local LLM' }),
                { status: 503, headers: { 'Content-Type': 'application/json' } }
            );
        }

        let body = null;
        try { body = await request.json(); } catch (_) { body = {}; }

        const id = ++llmRequestId;

        const sseStream = new ReadableStream({
            start(controller) {
                pendingLocalLlmStreams.set(id, { controller, closed: false });
                setTimeout(() => {
                    const stream = pendingLocalLlmStreams.get(id);
                    if (stream && !stream.closed) {
                        stream.closed = true;
                        pendingLocalLlmStreams.delete(id);
                        const frame = `event: error\ndata: ${JSON.stringify({ error: 'timeout' })}\n\n`;
                        stream.controller.enqueue(new TextEncoder().encode(frame));
                        stream.controller.close();
                    }
                }, 300_000); // 5 min, matches one-shot timeout
            },
            cancel() {
                const stream = pendingLocalLlmStreams.get(id);
                if (stream) {
                    stream.closed = true;
                    pendingLocalLlmStreams.delete(id);
                }
            },
        });

        clients[0].postMessage({
            type: 'local-llm-request',
            id,
            action: 'chat_stream',
            body,
        });

        return new Response(sseStream, {
            status: 200,
            headers: {
                'Content-Type': 'text/event-stream',
                'Cache-Control': 'no-cache',
                'Connection': 'keep-alive',
            },
        });
    }

    // Determine the action from the path
    let action;
    if (path === '/b/local-llm/api/chat') action = 'chat';
    else if (path === '/b/local-llm/api/models') action = 'models';
    else if (path === '/b/local-llm/api/load') action = 'load';
    else if (path === '/b/local-llm/api/unload') action = 'unload';
    else if (path === '/b/local-llm/api/status') action = 'status';
    else {
        return new Response(
            JSON.stringify({ error: 'not_found', message: 'Unknown local-llm endpoint' }),
            { status: 404, headers: { 'Content-Type': 'application/json' } }
        );
    }

    // Parse request body for POST endpoints
    let body = null;
    if (request.method === 'POST') {
        try { body = await request.json(); } catch (_) { body = {}; }
    }

    // Find a page client to forward to
    const clients = await self.clients.matchAll({ type: 'window', includeUncontrolled: false });
    if (clients.length === 0) {
        return new Response(
            JSON.stringify({ error: 'no_client', message: 'No active page — open the app in a tab to use local LLM' }),
            { status: 503, headers: { 'Content-Type': 'application/json' } }
        );
    }

    const id = ++llmRequestId;
    const responsePromise = new Promise((resolve, reject) => {
        pendingLocalLlm.set(id, { resolve, reject });
        // Timeout after 5 minutes (model loading can be slow)
        setTimeout(() => {
            if (pendingLocalLlm.has(id)) {
                pendingLocalLlm.delete(id);
                reject(new Error('Local LLM request timed out'));
            }
        }, 300_000);
    });

    // Send to the first available client
    clients[0].postMessage({
        type: 'local-llm-request',
        id,
        action,
        body,
    });

    try {
        const result = await responsePromise;
        const status = result.error ? (result.status || 500) : 200;
        return new Response(
            JSON.stringify(result.data || { error: result.error }),
            { status, headers: { 'Content-Type': 'application/json' } }
        );
    } catch (error) {
        return new Response(
            JSON.stringify({ error: 'bridge_error', message: String(error) }),
            { status: 500, headers: { 'Content-Type': 'application/json' } }
        );
    }
}

// Expose the local-llm bridge so Rust-side blocks can invoke it via
// bridge::local_llm_chat_stream. Matches the __gizzaCompleteAssetLoad
// pattern used for load-asset replies.
globalThis.__gizzaHandleLocalLlm = handleLocalLlm;

// ---------------------------------------------------------------------------
// Fetch handler
// ---------------------------------------------------------------------------

self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);
    // Only intercept same-origin requests
    if (url.origin !== self.location.origin) return;
    // Don't intercept requests for static assets served from CDN
    if (url.pathname === '/sw.js' ||
        url.pathname === '/loader.js' ||
        url.pathname === '/ai-bridge.js' ||
        url.pathname === '/manifest.json' ||
        url.pathname === '/index.html' ||
        url.pathname === '/' ||
        url.pathname.startsWith('/pkg/') ||
        url.pathname.startsWith('/sql-')) {
        return;
    }
    // Local-LLM: handle via postMessage bridge to main thread (WebGPU)
    if (url.pathname.startsWith('/b/local-llm/api/')) {
        event.respondWith(handleLocalLlm(event.request));
        return;
    }
    event.respondWith(handleFetch(event.request));
});

async function handleFetch(request) {
    try {
        await ensureInitialized();
        return await handle_request(request);
    } catch (error) {
        console.error('[gizza-ai] Error handling request:', error);
        return new Response(
            JSON.stringify({ error: 'internal_error', message: String(error) }),
            { status: 500, headers: { 'Content-Type': 'application/json' } }
        );
    }
}
