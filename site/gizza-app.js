// site/gizza-app.js — UI glue: settings, model loading, composer submit,
// SSE parsing. All state is in-memory — no persistence for MVP.

const history = []; // OpenAI-format messages.

const $ = (id) => document.getElementById(id);

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
    const text = el('span', { class: 'text' });
    bubble.appendChild(text);
    msgs.appendChild(bubble);
    scrollToBottom();
    return text;
}

function addToolRow(name, args) {
    const msgs = $('messages');
    const row = el('div', { class: 'tool-call' });
    row.appendChild(el('span', { class: 'spinner' }, '\u23f3'));
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
}

// --- Settings dialog ---
$('open-settings').addEventListener('click', () => $('settings').showModal());

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
    // Probe WebGPU; if missing, reveal the banner and disable Load model.
    const probe = await detectWebGPU();
    if (!probe.ok) {
        warn.hidden = false;
        const btn = $('load-model');
        btn.disabled = true;
        btn.title = 'WebGPU not available — see instructions above';
        btn.textContent = 'Load model (WebGPU required)';
    }
})();

// --- Model loading ---
//
// Posts to the agent block's /b/agent/load-model endpoint which wraps
// `wafer-run/llm` LLM_LOAD_MODEL. The server streams SSE frames of shape
// `event: load_progress data: {stage: "...", ...}` and terminates with
// `event: load_done data: {ok: boolean, error?: string}`.
$('load-model').addEventListener('click', async () => {
    const btn = $('load-model');
    btn.disabled = true;
    btn.textContent = 'Downloading\u2026';
    try {
        const resp = await fetch('/b/agent/load-model', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ model_id: window.__GIZZA_MODEL_ID }),
        });
        if (!resp.ok) throw new Error(`load-model HTTP ${resp.status}`);

        const reader = resp.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        let ok = false;
        let err = null;

        outer: while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            buffer += decoder.decode(value, { stream: true });
            let idx;
            while ((idx = buffer.indexOf('\n\n')) !== -1) {
                const frame = buffer.slice(0, idx);
                buffer = buffer.slice(idx + 2);
                let ev = '';
                const dataLines = [];
                for (const line of frame.split('\n')) {
                    if (line.startsWith('event:')) ev = line.slice(6).trim();
                    else if (line.startsWith('data:')) {
                        dataLines.push(line.slice(5).replace(/^ /, ''));
                    }
                }
                let data = {};
                try { data = JSON.parse(dataLines.join('\n')); } catch (_) {}
                if (ev === 'load_progress') {
                    btn.textContent = data.stage
                        ? `Downloading\u2026 ${data.stage}`
                        : 'Downloading\u2026';
                } else if (ev === 'load_done') {
                    ok = !!data.ok;
                    if (!ok) err = data.error || 'unknown error';
                    break outer;
                }
            }
        }

        if (!ok) throw new Error(err || 'load-model did not complete');
        btn.textContent = 'Ready';
        $('send').disabled = false;
    } catch (e) {
        const msg = String(e?.message ?? e);
        // WebLLM's compatible-GPU error is long and scary; surface the
        // per-browser setup panel instead of just dumping the message.
        if (/compatible GPU|WebGPU|gpu adapter/i.test(msg)) {
            const warn = $('webgpu-warning');
            if (warn) warn.hidden = false;
            btn.textContent = 'Load model (see WebGPU instructions)';
        } else {
            btn.textContent = `Error: ${msg}`;
        }
        btn.disabled = false;
    }
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

    let assistantText = '';
    const assistantEl = addAssistantBubble();
    const toolRows = new Map(); // tool-call id -> DOM row

    try {
        const resp = await fetch('/b/agent/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ user_message: text, messages: history.slice(0, -1) }),
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
    } catch (err) {
        assistantEl.textContent = `(error: ${err.message})`;
    } finally {
        input.disabled = false;
        $('send').disabled = false;
        input.focus();
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
            assistantEl.textContent = assistantText;
            scrollToBottom();
        } else if (event === 'tool_call') {
            const row = addToolRow(payload?.name ?? '?', payload?.arguments ?? '');
            toolRows.set(payload?.id ?? crypto.randomUUID(), row);
        } else if (event === 'tool_result') {
            const row = toolRows.get(payload?.id);
            if (row) updateToolRow(row, !payload?.error, payload?.result ?? payload?.error ?? '');
        } else if (event === 'done') {
            // Nothing to do — reader finished.
        }
    }
});
