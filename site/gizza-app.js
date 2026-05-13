// site/gizza-app.js — UI glue: settings, model loading, composer submit,
// SSE parsing. All state is in-memory — no persistence for MVP.

import { renderToolAttachment } from './render.js';
import {
    addPending,
    removePending,
    clearPending,
    getPending,
    renderChips,
} from './pending.js';
import { loadEngine, unloadEngine } from '/webllm-engine.js';
import { openPicker } from '/model-picker.js';

const history = []; // OpenAI-format messages.

const $ = (id) => document.getElementById(id);

async function blobToBase64(blob) {
    return await new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => {
            const result = reader.result; // "data:<mime>;base64,<payload>"
            const comma = result.indexOf(',');
            resolve(comma >= 0 ? result.slice(comma + 1) : result);
        };
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(blob);
    });
}

// Local-storage key for the user's chosen WebLLM model id. Set whenever the
// model picker changes; read at page-init to restore the previous choice.
const SELECTED_MODEL_KEY = 'gizza.selectedModel';

// Read the selected model, falling back to the server-rendered default
// (window.__GIZZA_MODEL_ID) when nothing's stored. Always returns a non-empty
// string so callers can pass it directly to the agent endpoints.
function selectedModelId() {
    const stored = localStorage.getItem(SELECTED_MODEL_KEY);
    if (stored && stored.trim()) return stored;
    return window.__GIZZA_MODEL_ID;
}

// Brand text fix-ups — the maud HTML still ships "gizza-ai"; we override
// here so the wordmark and tab title both read "gizza.ai" without a WASM
// rebuild.
document.title = 'gizza.ai';
{
    const wordmark = document.querySelector('sa-header h1');
    if (wordmark) wordmark.textContent = 'gizza.ai';
}

// Move the settings cog from the header into the composer's action row,
// just before the send button. The Lucide cog icon and ghost-button styling
// are applied by gizza.css via #composer #open-settings (mask-image on
// ::before — the button itself is empty in the maud HTML).
{
    const cog = document.getElementById('open-settings');
    const send = document.getElementById('send');
    if (cog && send && cog.parentElement !== send.parentElement) {
        send.parentElement.insertBefore(cog, send);
    }
}

function el(tag, attrs = {}, text = '') {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
    if (text) e.textContent = text;
    return e;
}

function scrollToBottom() {
    const m = $('messages');
    m.scrollTop = m.scrollHeight;
}

function addUserBubble(text) {
    const msgs = $('messages');
    const empty = msgs.querySelector('.empty');
    if (empty) empty.remove();
    const bubble = el('div', { class: 'bubble user' }, text);
    msgs.appendChild(bubble);
    scrollToBottom();
}

function addAssistantBubble() {
    const msgs = $('messages');
    const bubble = el('div', { class: 'bubble assistant' });
    const text = el('span', { class: 'text markdown' });
    bubble.appendChild(text);
    msgs.appendChild(bubble);
    scrollToBottom();
    return text;
}

// Render assistant text as markdown with syntax-highlighted code blocks.
// `raw` is the full accumulated content so far (we re-render on each token —
// marked is fast enough that flicker isn't noticeable for typical chat
// outputs, and partial code-fence handling stays correct because marked sees
// the full buffer on each call).
//
// Safety: assistant text is from a model we run, but model output can still
// contain HTML-looking strings. marked's default options HTML-escape inline
// content, so this is safe as long as we don't pass `{ sanitize: false }` and
// don't enable `mangle: false` with raw HTML allowed. Marked 13 is
// HTML-escape-by-default — no extra sanitiser needed.
function renderAssistantContent(node, raw) {
    if (typeof marked === 'undefined') {
        node.textContent = raw;
        return;
    }
    node.innerHTML = marked.parse(raw, { breaks: true, gfm: true });
    if (typeof hljs !== 'undefined') {
        for (const block of node.querySelectorAll('pre code')) {
            try { hljs.highlightElement(block); } catch (_e) { /* ignore */ }
        }
    }
}

function addToolRow(name, args) {
    const msgs = $('messages');
    const row = el('div', { class: 'tool-call' });
    row.appendChild(el('span', { class: 'spinner' }));
    row.appendChild(el('code', {}, `${name}(${args})`));
    msgs.appendChild(row);
    scrollToBottom();
    return row;
}

function updateToolRow(row, ok, result) {
    const spinner = row.querySelector('.spinner');
    if (spinner) spinner.textContent = ok ? '\u2713' : '\u2717';
    const resSpan = el('span', { class: 'result' }, ` \u2192 ${result}`);
    row.appendChild(resSpan);
    row.classList.add(ok ? 'is-done' : 'is-error');
}

// PR 5: render an inline [Yes]/[No] confirmation chip pair into the
// assistant bubble. `payload` is the SSE `confirm` event body:
// `{question, yes: {cmd, params}, no: null}`. The Yes button hides the
// chips (parent caller replaces bubble children to stream the dispatch
// response in their place); No just dismisses locally.
function renderConfirmChips(bubble, payload, onYes, onNo) {
    const question = String(payload?.question ?? 'Run this command?');
    const wrap = el('div', { class: 'confirm-chips' });
    wrap.appendChild(el('p', { class: 'confirm-question' }, question));
    const buttons = el('div', { class: 'confirm-buttons' });
    const yes = el('button', { class: 'confirm-yes', type: 'button' }, 'Yes');
    const no = el('button', { class: 'confirm-no', type: 'button' }, 'No');
    yes.addEventListener('click', () => onYes && onYes());
    no.addEventListener('click', () => onNo && onNo());
    buttons.appendChild(yes);
    buttons.appendChild(no);
    wrap.appendChild(buttons);
    bubble.appendChild(wrap);
    scrollToBottom();
}

// PR 5: handle a Yes click on a confirm chip pair. Posts the
// pre-extracted `confirm_yes: {cmd, params}` to /b/agent/chat, streams
// the SSE response, and renders tool_result events into the same
// assistantEl bubble the chip pair lived in.
async function streamConfirmYes(yes, assistantEl) {
    if (!yes || typeof yes !== 'object' || !yes.cmd) {
        assistantEl.appendChild(document.createTextNode('(invalid confirm payload)'));
        return;
    }
    try {
        const resp = await fetch('/b/agent/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ confirm_yes: yes, model_id: selectedModelId() }),
        });
        if (!resp.ok) throw new Error(`agent HTTP ${resp.status}`);
        const reader = resp.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        let text = '';
        while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            buffer += decoder.decode(value, { stream: true });
            let idx;
            while ((idx = buffer.indexOf('\n\n')) !== -1) {
                const frame = buffer.slice(0, idx);
                buffer = buffer.slice(idx + 2);
                const lines = frame.split('\n');
                let event = '';
                let data = '';
                for (const line of lines) {
                    if (line.startsWith('event:')) event = line.slice(6).trim();
                    else if (line.startsWith('data:')) data += line.slice(5).replace(/^ /, '');
                }
                if (!event) continue;
                let payload;
                try { payload = JSON.parse(data); } catch { payload = data; }
                if (event === 'tool_result') {
                    const summary = payload?.result ?? payload?.error ?? '';
                    if (summary && !payload?.for_ui) {
                        text += summary;
                        renderAssistantContent(assistantEl, text);
                    }
                    if (payload?.for_ui) {
                        renderToolAttachment(assistantEl, payload.for_ui);
                    }
                    scrollToBottom();
                } else if (event === 'done' && payload?.reason === 'error') {
                    text = `_(agent error: ${payload?.error || 'unknown'})_`;
                    renderAssistantContent(assistantEl, text);
                }
            }
        }
    } catch (err) {
        assistantEl.appendChild(document.createTextNode(`(error: ${err.message})`));
    }
}

// --- Drag-drop + file-picker upload wiring ---

function showUploadError(text) {
    const strip = document.getElementById('upload-chips');
    strip.replaceChildren();
    const err = document.createElement('span');
    err.className = 'upload-error';
    err.textContent = text;
    strip.appendChild(err);
    strip.classList.remove('empty');
}

function refreshChips() {
    const strip = document.getElementById('upload-chips');
    renderChips(strip);
    strip.querySelectorAll('button.remove').forEach((btn) => {
        const chip = btn.closest('.chip');
        const id = chip?.getAttribute('data-id');
        if (id) {
            btn.addEventListener('click', () => {
                removePending(id);
                refreshChips();
            });
        }
    });
}

function ingestFiles(fileList) {
    let firstError = null;
    for (const f of fileList) {
        const r = addPending(f);
        if (!r.ok && !firstError) firstError = r.error;
    }
    if (firstError && getPending().length === 0) {
        showUploadError(firstError);
        return;
    }
    refreshChips();
    if (firstError) {
        console.warn('Upload partially rejected:', firstError);
    }
}

function setupUploads() {
    const attach = document.getElementById('attach');
    const picker = document.getElementById('file-picker');
    if (!attach || !picker) return;
    attach.addEventListener('click', () => picker.click());
    picker.addEventListener('change', () => {
        ingestFiles(picker.files);
        picker.value = '';
    });

    const overlay = document.createElement('div');
    overlay.id = 'drop-overlay';
    overlay.hidden = true;
    overlay.innerHTML =
        '<div class="drop-overlay-inner">Drop image or video to attach.</div>';
    document.body.appendChild(overlay);

    let dragDepth = 0;
    document.addEventListener('dragenter', (e) => {
        if (!e.dataTransfer || !Array.from(e.dataTransfer.types).includes('Files')) return;
        e.preventDefault();
        dragDepth += 1;
        overlay.hidden = false;
    });
    document.addEventListener('dragover', (e) => {
        if (!e.dataTransfer || !Array.from(e.dataTransfer.types).includes('Files')) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = 'copy';
    });
    document.addEventListener('dragleave', (e) => {
        if (!e.dataTransfer || !Array.from(e.dataTransfer.types).includes('Files')) return;
        dragDepth = Math.max(0, dragDepth - 1);
        if (dragDepth === 0) overlay.hidden = true;
    });
    document.addEventListener('drop', (e) => {
        if (!e.dataTransfer || e.dataTransfer.files.length === 0) return;
        e.preventDefault();
        dragDepth = 0;
        overlay.hidden = true;
        ingestFiles(e.dataTransfer.files);
    });
}

setupUploads();

// --- Brand mascot state machine ---
//
// The mascot is a 3-layer composite: a still PNG (eyes-less), a video that
// overlays it during animation, and pupil sprites that track the user's
// cursor while a still is showing.
//
// Modes / poses:
//   resting       — gis_no_eyes.png + pupils (boot state)
//   video-idle    — gis_video_idle.mp4 (the "gis a job" reveal), no pupils
//   sign          — gis_a_job_no_eyes.png + pupils (after idle animation)
//   typing        — gis_video_typing_loop.mp4 looping, no pupils
//   typing-finish — gis_video_typing_finish.mp4 (tail) once, no pupils
//
// Flow: resting → 10s no activity → video-idle → ended → sign (stays).
// On composer submit: → typing. On finally: → typing-finish → ended → sign.
const brandMascot = document.querySelector('.brand-mascot');
if (brandMascot) {
    const brandStill = brandMascot.querySelector('.brand-still');
    const brandVideo = brandMascot.querySelector('.brand-video');
    const eyes = brandMascot.querySelectorAll('.brand-eye');

    const RESTING_SRC = '/gis_no_eyes.png';
    const SIGN_SRC = '/gis_a_job_no_eyes.png';
    const IDLE_VIDEO = '/gis_video_idle.mp4';
    const TYPING_LOOP_SRC = '/gis_video_typing_loop.mp4';
    const TYPING_FINISH_SRC = '/gis_video_typing_finish.mp4';
    const IDLE_DELAY_MS = 10_000;

    let brandMode = 'resting';
    let idleTimer = null;

    const absUrl = (rel) => new URL(rel, location.href).href;
    const videoSrcIs = (rel) => brandVideo.currentSrc === absUrl(rel);

    function clearIdleTimer() {
        if (idleTimer) { clearTimeout(idleTimer); idleTimer = null; }
    }

    function showStill(src, pose) {
        brandStill.src = src;
        brandStill.hidden = false;
        brandVideo.hidden = true;
        try { brandVideo.pause(); } catch (_) {}
        brandMascot.dataset.pose = pose;
    }

    function showVideo(src, { loop }) {
        brandStill.hidden = true;
        brandVideo.hidden = false;
        brandVideo.loop = loop;
        brandMascot.dataset.pose = 'video';
        const start = () => {
            try { brandVideo.currentTime = 0; } catch (_) {}
            brandVideo.play().catch(() => {});
        };
        if (!videoSrcIs(src)) {
            brandVideo.src = src;
            brandVideo.addEventListener('loadedmetadata', start, { once: true });
        } else {
            start();
        }
    }

    function enterResting() {
        brandMode = 'resting';
        showStill(RESTING_SRC, 'resting');
        // After 10s of no activity, play the "gis a job" reveal video.
        clearIdleTimer();
        idleTimer = setTimeout(() => enterVideoIdle(), IDLE_DELAY_MS);
    }

    function enterVideoIdle() {
        if (brandMode === 'typing' || brandMode === 'typing-finish') return;
        brandMode = 'video-idle';
        clearIdleTimer();
        showVideo(IDLE_VIDEO, { loop: false });
    }

    function enterSign() {
        brandMode = 'sign';
        clearIdleTimer();
        showStill(SIGN_SRC, 'sign');
    }

    function startTyping() {
        clearIdleTimer();
        brandMode = 'typing';
        showVideo(TYPING_LOOP_SRC, { loop: true });
    }

    function stopTyping() {
        if (brandMode !== 'typing') return;
        brandMode = 'typing-finish';
        showVideo(TYPING_FINISH_SRC, { loop: false });
    }

    brandVideo.addEventListener('ended', () => {
        if (brandMode === 'video-idle' || brandMode === 'typing-finish') {
            enterSign();
        }
    });

    // ─── Pupil mouse-tracking ──────────────────────────────────────────────
    // Each eye socket has overflow:hidden — the pupil image translates
    // within its socket based on cursor angle/distance, mirroring the
    // solobase-site Hero approach.
    function onMouseMove(e) {
        if (brandMascot.dataset.pose === 'video') return;
        requestAnimationFrame(() => {
            for (const eye of eyes) {
                const socket = eye.parentElement;
                const sr = socket.getBoundingClientRect();
                const er = eye.getBoundingClientRect();
                const cx = sr.left + sr.width / 2;
                const cy = sr.top + sr.height / 2;
                const dx = e.clientX - cx;
                const dy = e.clientY - cy;
                const angle = Math.atan2(dy, dx);
                const distance = Math.hypot(dx, dy);
                const maxX = (sr.width - er.width) / 2;
                const maxY = (sr.height - er.height) / 2;
                const scale = Math.min(distance / 200, 1);
                const mx = Math.cos(angle) * maxX * scale;
                const my = Math.sin(angle) * maxY * scale;
                eye.style.transform = `translate(${mx}px, ${my}px)`;
            }
        });
    }
    function onMouseLeave() {
        for (const eye of eyes) eye.style.transform = 'translate(0, 0)';
    }
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseleave', onMouseLeave);

    window.__brandStartTyping = startTyping;
    window.__brandStopTyping = stopTyping;

    enterResting();
}

// Progress card \u2014 created lazily inside the Settings dialog while the model
// downloads. Holds the verbose stage message so the Load model button text
// can stay short.
function setLoadProgress(text, percent, isError = false) {
    let card = $('load-progress');
    if (!card) {
        card = el('div', { id: 'load-progress', class: 'progress-card' });
        const stage = el('div', { class: 'progress-stage' });
        const bar = el('div', { class: 'progress-bar' });
        const fill = el('div', { class: 'progress-bar-fill' });
        bar.appendChild(fill);
        card.appendChild(stage);
        card.appendChild(bar);
        // Prefer to show progress inline in the chat surface so it's visible
        // when the user kicked off the load from the empty-state CTA (the
        // settings dialog is closed in that flow). Fall back to next-to the
        // settings dialog's load button when the empty state has been
        // dismissed.
        const empty = document.querySelector('#messages .empty');
        const loadBtn = $('open-model-picker') || $('load-model');
        if (empty) {
            empty.replaceChildren(card);
        } else if (loadBtn) {
            loadBtn.parentNode.insertBefore(card, loadBtn);
        }
    }
    card.querySelector('.progress-stage').textContent = text || '';
    const bar = card.querySelector('.progress-bar');
    const fill = card.querySelector('.progress-bar-fill');
    if (typeof percent === 'number' && !isNaN(percent)) {
        const pct = Math.max(0, Math.min(100, percent));
        fill.style.width = `${pct}%`;
        bar.classList.remove('is-indeterminate');
    } else {
        bar.classList.add('is-indeterminate');
    }
    card.classList.toggle('is-error', !!isError);
}

function clearLoadProgress() {
    const card = $('load-progress');
    if (card) card.remove();
    // If the progress card had taken over the empty-state, swap in a
    // ready-state hint so the chat surface isn't left blank.
    const empty = document.querySelector('#messages .empty');
    if (empty && empty.childElementCount === 0 && !empty.textContent.trim()) {
        empty.textContent = 'Ready — ask anything below.';
    }
}

// Pull a percent like "15% completed" out of the runtime's stage message.
function parsePercentFromStage(stage) {
    if (!stage) return null;
    const m = stage.match(/(\d+(?:\.\d+)?)\s*%/);
    return m ? parseFloat(m[1]) : null;
}

// --- Settings dialog ---
// ⋯ button → toggle popup menu anchored above the button.
$('open-settings').addEventListener('click', (e) => {
    e.stopPropagation();
    const menu = $('composer-menu');
    const btn = $('open-settings');
    if (!menu || !btn) return;
    if (!menu.hidden) {
        menu.hidden = true;
        return;
    }
    const rect = btn.getBoundingClientRect();
    menu.style.left = `${rect.left}px`;
    menu.style.top  = `${rect.top - 4}px`;
    menu.style.transform = 'translateY(-100%)';
    menu.hidden = false;
});
// Click-outside closes the menu.
document.addEventListener('click', (e) => {
    const menu = $('composer-menu');
    if (!menu || menu.hidden) return;
    if (e.target.closest('#composer-menu') || e.target.closest('#open-settings')) return;
    menu.hidden = true;
});
$('menu-info')?.addEventListener('click', () => {
    $('composer-menu').hidden = true;
    $('info-dialog')?.showModal();
});
$('menu-webgpu')?.addEventListener('click', () => {
    $('composer-menu').hidden = true;
    $('settings')?.showModal();
});
$('menu-discord')?.addEventListener('click', () => {
    // Native <a target="_blank"> handles the navigation; we just close the menu.
    $('composer-menu').hidden = true;
});
$('menu-clear')?.addEventListener('click', () => {
    $('composer-menu').hidden = true;
    $('clear-convo')?.click();
});
$('menu-close')?.addEventListener('click', () => {
    $('composer-menu').hidden = true;
});

// --- Model picker ---
//
// Legacy populateModelPicker — kept as a no-op so any old call sites don't
// throw. Selection is now driven entirely by openPicker() (model-picker.js),
// invoked from rewriteSettingsDialog and rewriteEmptyState below.
async function populateModelPicker() {
    // Picker is driven by openPicker() now; the legacy <select> is removed
    // from the DOM by rewriteSettingsDialog() at boot. This function is kept
    // for callers but is intentionally empty.
}
populateModelPicker();

// --- WebGPU detection + per-browser setup instructions ---
async function detectWebGPU() {
    if (!('gpu' in navigator)) return { ok: false, reason: 'no-api' };
    try {
        const adapter = await navigator.gpu.requestAdapter();
        if (!adapter) return { ok: false, reason: 'no-adapter' };
        return { ok: true };
    } catch (e) {
        return { ok: false, reason: 'error', message: String(e?.message ?? e) };
    }
}

function detectBrowserTab() {
    const ua = navigator.userAgent || '';
    // Edge identifies as "Edg/" — put before Chrome (which it also matches).
    // Both use Chromium's WebGPU path so they share the same tab.
    if (/Edg\//.test(ua) || /Chrome\//.test(ua)) return 'chrome';
    if (/Firefox\//.test(ua)) return 'firefox';
    if (/Safari\//.test(ua) && !/Chrome\//.test(ua)) return 'safari';
    return 'other';
}

function activateTab(name) {
    const warn = $('webgpu-warning');
    if (!warn) return;
    for (const tab of warn.querySelectorAll('.tab')) {
        tab.classList.toggle('active', tab.dataset.tab === name);
    }
    for (const panel of warn.querySelectorAll('.tab-panel')) {
        panel.hidden = panel.dataset.tab !== name;
    }
}

(async () => {
    const warn = $('webgpu-warning');
    if (!warn) return;
    // Wire tab clicks.
    for (const tab of warn.querySelectorAll('.tab')) {
        tab.addEventListener('click', () => activateTab(tab.dataset.tab));
    }
    // Wire click-to-copy on internal-URL pills (chrome://, about:config, etc.
    // which browsers block from regular pages).
    for (const pill of warn.querySelectorAll('.copy-url')) {
        pill.addEventListener('click', async () => {
            const url = pill.dataset.url;
            if (!url) return;
            try {
                await navigator.clipboard.writeText(url);
                const prev = pill.textContent;
                pill.textContent = 'Copied ✓';
                pill.classList.add('copied');
                setTimeout(() => {
                    pill.textContent = prev;
                    pill.classList.remove('copied');
                }, 1500);
            } catch {
                pill.title = 'Could not copy — select and copy manually';
            }
        });
    }
    // Auto-select the tab that matches the user's browser.
    activateTab(detectBrowserTab());
    // Probe WebGPU; if missing, reveal the banner and disable the load button.
    // The button may be #load-model (before rewriteSettingsDialog runs) or
    // #open-model-picker (after) — check both.
    const probe = await detectWebGPU();
    if (!probe.ok) {
        warn.hidden = false;
        const btn = $('open-model-picker') || $('load-model');
        if (btn) {
            btn.disabled = true;
            btn.title = 'WebGPU not available — see instructions above';
            btn.textContent = 'Load model (WebGPU required)';
        }
    }
})();

// --- Model loading ---
//
// Calls loadEngine() directly in the window. WebLLM's CreateMLCEngine
// runs here and populates _engine in webllm-engine.js (module-scoped).
// Subsequent chat goes through the SW (gizza-app.js → /b/agent/chat
// → BrowserLlmService::chat → postMessage page) and reads the same
// _engine, because ESM module-scoped state is shared in a realm.
// Drives WebLLM's CreateMLCEngine directly in the window via the
// `loadEngine` export from /webllm-engine.js. Bypasses the SW round-trip
// because Chrome kills FetchEvent.respondWith() at ~5 min \u2014 see
// docs/superpowers/handoffs/2026-05-07-gizza-ai-model-load-page-direct-handoff.md.

// Tracks the model id of the most recently loaded (or currently loading)
// engine so the picker can show the active model. Updated on each successful
// load.
let _loadedModelId = null;

function getCurrentEngineModelId() {
    return _loadedModelId;
}

async function startModelLoad(modelId) {
    // If no modelId passed, fall back to whatever is persisted in localStorage.
    const id = modelId || selectedModelId();
    const btn = $('open-model-picker') || $('load-model');
    if (btn) {
        btn.disabled = true;
        btn.textContent = 'Downloading\u2026';
    }
    setLoadProgress('Starting\u2026', null);
    try {
        await loadEngine(id, (text) => {
            if (btn) btn.textContent = 'Downloading\u2026';
            setLoadProgress(text || 'Downloading\u2026', parsePercentFromStage(text));
        });
        _loadedModelId = id;
        if (btn) btn.textContent = 'Ready';
        clearLoadProgress();
        $('send').disabled = false;
    } catch (e) {
        const msg = String(e?.message ?? e);
        if (/compatible GPU|WebGPU|gpu adapter/i.test(msg)) {
            const warn = $('webgpu-warning');
            if (warn) warn.hidden = false;
            if (btn) btn.textContent = 'Load model';
            clearLoadProgress();
        } else {
            if (btn) btn.textContent = 'Try again';
            setLoadProgress(msg, null, true);
        }
        if (btn) btn.disabled = false;
    }
}

// --- Picker overlay integration ---

async function launchPicker() {
    const mod = await import('https://cdn.jsdelivr.net/npm/@mlc-ai/web-llm@0.2.74/+esm');
    const prebuiltList = mod?.prebuiltAppConfig?.model_list || [];
    const result = await openPicker({
        prebuiltList,
        currentModelId: getCurrentEngineModelId(),
    });
    if (!result?.model_id) return;
    if (result.mode === 'download') {
        // Cache weights to IndexedDB without making this the active engine.
        // Implemented as load-then-unload via the page-direct engine — keeps
        // weights cached locally; existing _engine is replaced (single-slot
        // WebGPU constraint).
        setLoadProgress(`Downloading ${result.model_id}…`, null);
        try {
            await loadEngine(result.model_id, (text) => {
                setLoadProgress(text || 'Downloading…', parsePercentFromStage(text));
            });
            await unloadEngine?.();
            _loadedModelId = null;
            setLoadProgress(`${result.model_id} cached.`, 100);
            setTimeout(clearLoadProgress, 2000);
        } catch (e) {
            setLoadProgress(`Download failed: ${e?.message ?? e}`, null, true);
        }
        return;
    }
    localStorage.setItem(SELECTED_MODEL_KEY, result.model_id);
    await startModelLoad(result.model_id);
}

// Settings → Choose model (closes the dialog so the full-screen picker takes over).
$('open-model-picker')?.addEventListener('click', async (e) => {
    e.preventDefault();
    document.getElementById('settings').close();
    await launchPicker();
});

// Empty-state CTA on the chat surface. The empty div is replaced after the user
// clears a conversation — guard for the case where boot races with that path.
$('empty-state-cta')?.addEventListener('click', () => launchPicker());

// Composer brain icon — leftmost button, opens the model picker directly.
$('open-brain-picker')?.addEventListener('click', (e) => {
    e.preventDefault();
    launchPicker();
});

// --- Clear conversation ---
$('clear-convo').addEventListener('click', () => {
    history.length = 0;
    $('messages').replaceChildren(el('div', { class: 'empty' }, 'Conversation cleared.'));
});

// --- Composer ---
$('composer').addEventListener('submit', async (e) => {
    e.preventDefault();
    const input = $('user-input');
    const text = input.value.trim();
    if (!text) return;

    addUserBubble(text);
    history.push({ role: 'user', content: text });
    input.value = '';
    $('send').disabled = true;
    input.disabled = true;
    window.__brandStartTyping?.();

    let assistantText = '';
    const assistantEl = addAssistantBubble();
    const toolRows = new Map(); // tool-call id -> DOM row

    const pendingUploads = getPending();
    const uploads = await Promise.all(
        pendingUploads.map(async (p) => ({
            id: p.id,
            mime: p.mime,
            filename: p.filename,
            bytes_base64: await blobToBase64(p.blob),
        })),
    );

    let roundTripCompleted = false;
    try {
        const resp = await fetch('/b/agent/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                user_message: text,
                messages: history.slice(0, -1),
                model_id: selectedModelId(),
                uploads,
            }),
        });
        if (!resp.ok) throw new Error(`agent HTTP ${resp.status}`);

        const reader = resp.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            buffer += decoder.decode(value, { stream: true });
            let idx;
            while ((idx = buffer.indexOf('\n\n')) !== -1) {
                const frame = buffer.slice(0, idx);
                buffer = buffer.slice(idx + 2);
                processFrame(frame);
            }
        }
        if (buffer.trim()) processFrame(buffer);

        if (assistantText) {
            history.push({ role: 'assistant', content: assistantText });
        }
        roundTripCompleted = true;
    } catch (err) {
        assistantEl.textContent = `(error: ${err.message})`;
    } finally {
        window.__brandStopTyping?.();
        input.disabled = false;
        $('send').disabled = false;
        input.focus();
        // Clear pending uploads only on a successful round-trip — leave chips
        // visible on network/parse error so the user can retry without
        // re-attaching the files.
        if (roundTripCompleted) {
            clearPending();
            refreshChips();
        }
    }

    function processFrame(frame) {
        const lines = frame.split('\n');
        let event = '';
        let data = '';
        for (const line of lines) {
            if (line.startsWith('event:')) event = line.slice(6).trim();
            else if (line.startsWith('data:')) data += line.slice(5).replace(/^ /, '');
        }
        if (!event) return;
        let payload;
        try { payload = JSON.parse(data); } catch { payload = data; }

        if (event === 'token' && payload?.delta) {
            assistantText += payload.delta;
            renderAssistantContent(assistantEl, assistantText);
            scrollToBottom();
        } else if (event === 'tool_call') {
            const row = addToolRow(payload?.name ?? '?', payload?.arguments ?? '');
            toolRows.set(payload?.id ?? crypto.randomUUID(), row);
        } else if (event === 'tool_result') {
            const row = toolRows.get(payload?.id);
            if (row) {
                updateToolRow(row, !payload?.error, payload?.result ?? payload?.error ?? '');
                renderToolAttachment(row, payload?.for_ui);
            } else {
                // Slash-command path: no preceding tool_call event because
                // the user-typed `/<cmd>` IS the call. Render the result
                // (and any inline-media attachment) into the assistant
                // bubble. The textual summary is purely informational —
                // _for_ui carries the image/video for the user.
                const summary = payload?.result ?? payload?.error ?? '';
                if (summary && !payload?.for_ui) {
                    assistantText += summary;
                    renderAssistantContent(assistantEl, assistantText);
                }
                if (payload?.for_ui) {
                    renderToolAttachment(assistantEl, payload.for_ui);
                }
                scrollToBottom();
            }
        } else if (event === 'confirm') {
            // PR 5: ambiguous slash-command params — backend asks the user
            // to confirm before dispatching. Render a question line and
            // [Yes]/[No] chips into the assistant bubble.
            renderConfirmChips(assistantEl, payload, () => {
                // Yes: re-fire chat with confirm_yes. The current
                // assistantEl is the chip-carrying bubble; replace its
                // contents with a spinner and let the response stream into
                // it via the existing assistantText accumulator.
                assistantEl.replaceChildren();
                assistantText = '';
                streamConfirmYes(payload?.yes, assistantEl);
            }, () => {
                // No: dismiss locally, no backend round-trip.
                assistantEl.replaceChildren();
                assistantText = '_(cancelled)_';
                renderAssistantContent(assistantEl, assistantText);
            });
        } else if (event === 'done') {
            // Surface terminal errors and empty-stop states so the user
            // isn't left staring at a blank bubble. Token paths overwrite
            // assistantText already, so this only fires when nothing
            // streamed in.
            const reason = payload?.reason;
            const errText = payload?.error;
            if (reason === 'error') {
                assistantText = `_(agent error: ${errText || 'unknown'})_`;
                renderAssistantContent(assistantEl, assistantText);
            } else if (reason === 'max_rounds_exceeded') {
                if (!assistantText) {
                    assistantText = '_(stopped: max tool-use rounds exceeded)_';
                    renderAssistantContent(assistantEl, assistantText);
                }
            } else if (reason === 'stop' && !assistantText && toolRows.size === 0
                && !assistantEl.firstChild) {
                // Empty bubble — no LLM tokens, no tool row, no slash result.
                assistantText = '_(model returned no content)_';
                renderAssistantContent(assistantEl, assistantText);
            }
        }
    }
});
