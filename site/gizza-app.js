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

// --- Model loading ---
$('load-model').addEventListener('click', async () => {
    const btn = $('load-model');
    btn.disabled = true;
    btn.textContent = 'Downloading\u2026';
    try {
        await window.gizzaAI.loadModel(window.__GIZZA_MODEL_ID, (progress) => {
            btn.textContent = progress.text || `Downloading\u2026 ${Math.round((progress.progress || 0) * 100)}%`;
        });
        btn.textContent = 'Ready';
        $('send').disabled = false;
    } catch (e) {
        btn.textContent = `Error: ${e?.message ?? e}`;
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
