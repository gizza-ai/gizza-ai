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
// rebuild. The wordmark is now the shared gizza-chrome header's brand <h1>
// (id="brand-wordmark"); the old `sa-header h1` selector is gone with the
// external web component.
document.title = 'gizza.ai';
{
    const wordmark = document.getElementById('brand-wordmark');
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

function el(tag, attrs = {}, children = '') {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
        if (k === 'onClick' && typeof v === 'function') e.addEventListener('click', v);
        else if (k === 'onInput' && typeof v === 'function') e.addEventListener('input', v);
        else if (k === 'onChange' && typeof v === 'function') e.addEventListener('change', v);
        else if (v === true) e.setAttribute(k, '');
        else if (v !== false && v != null) e.setAttribute(k, v);
    }
    if (Array.isArray(children)) {
        for (const c of children) {
            if (c == null) continue;
            e.appendChild(typeof c === 'string' ? document.createTextNode(c) : c);
        }
    } else if (children instanceof Node) {
        e.appendChild(children);
    } else if (children) {
        e.textContent = children;
    }
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

// Playful cycling status while we wait for the model. Returns a handle whose
// `.stop()` clears the timer and the placeholder. If the node has been written
// to before stop(), we leave the content alone (caller has rendered real text).
const THINKING_WORDS = [
    'thinking', 'pondering', 'noodling', 'cooking', 'untangling',
    'brewing', 'mulling', 'ruminating', 'cogitating', 'plotting',
    'figuring', 'churning', 'simmering', 'percolating',
];
function startThinking(node, opening = null) {
    if (!node) return { stop: () => {} };
    const pick = (i) => `${opening || THINKING_WORDS[i % THINKING_WORDS.length]}`;
    let i = Math.floor(Math.random() * THINKING_WORDS.length);
    const span = el('span', { class: 'thinking' });
    const word = el('span', { class: 'thinking-word' }, pick(i));
    const dots = el('span', { class: 'thinking-dots' }, '…');
    span.appendChild(word);
    span.appendChild(dots);
    node.replaceChildren(span);
    const timer = setInterval(() => {
        i++;
        word.textContent = pick(i);
    }, 1500);
    let stopped = false;
    return {
        stop: () => {
            if (stopped) return;
            stopped = true;
            clearInterval(timer);
            // If the placeholder is still the only child, clear it so the
            // caller can write real content. If the caller already wrote, the
            // span will have been replaced — nothing to do.
            if (node.firstChild === span) node.replaceChildren();
        },
    };
}

// Modal popup with two tabs (Input | Output) for the raw JSON behind a
// formatted slash result. Lets users see what the LLM extracted from their
// message vs what the skill returned — useful for debugging cases like a
// /calculator typo where the LLM read 't2' as a variable.
function showRawPopup({ input, output, title = 'Raw' } = {}) {
    const pretty = (v) => {
        if (v == null) return '(none)';
        if (typeof v === 'string') {
            try { return JSON.stringify(JSON.parse(v), null, 2); } catch { return v; }
        }
        try { return JSON.stringify(v, null, 2); } catch { return String(v); }
    };
    const inputText = pretty(input);
    const outputText = pretty(output);

    const dlg = el('dialog', { class: 'raw-popup' });

    // Header with tabs.
    let activeTab = output != null ? 'output' : 'input';
    const inputBtn = el('button', {
        class: 'raw-popup-tab',
        type: 'button',
        onClick: () => switchTab('input'),
    }, 'Input');
    const outputBtn = el('button', {
        class: 'raw-popup-tab',
        type: 'button',
        onClick: () => switchTab('output'),
    }, 'Output');
    const closeBtn = el('button', {
        class: 'raw-popup-close',
        'aria-label': 'Close',
        onClick: () => dlg.close(),
    }, '✕');
    const tabs = el('div', { class: 'raw-popup-tabs' }, [inputBtn, outputBtn]);
    const header = el('div', { class: 'raw-popup-header' }, [
        el('h3', {}, title),
        tabs,
        closeBtn,
    ]);

    const codeNode = el('code', {}, activeTab === 'output' ? outputText : inputText);
    const body = el('pre', { class: 'raw-popup-body' }, codeNode);

    const copyBtn = el('button', {
        class: 'raw-popup-copy',
        onClick: async () => {
            const txt = activeTab === 'output' ? outputText : inputText;
            try { await navigator.clipboard.writeText(txt); } catch (_e) {}
            const ok = el('span', { class: 'raw-popup-copied' }, 'Copied');
            footer.appendChild(ok);
            setTimeout(() => ok.remove(), 1200);
        },
    }, 'Copy');
    const footer = el('div', { class: 'raw-popup-footer' }, [copyBtn]);

    function switchTab(name) {
        activeTab = name;
        codeNode.textContent = name === 'output' ? outputText : inputText;
        inputBtn.classList.toggle('is-active', name === 'input');
        outputBtn.classList.toggle('is-active', name === 'output');
    }
    switchTab(activeTab);

    dlg.appendChild(header);
    dlg.appendChild(body);
    dlg.appendChild(footer);
    document.body.appendChild(dlg);
    dlg.addEventListener('close', () => dlg.remove());
    dlg.showModal();
}

// Append a small {} icon button to a bubble that opens the raw popup on click.
// `input` is what the LLM dispatched to the skill (extracted params); `output`
// is what the skill returned. Both are stringified for the popup.
function appendRawButton(bubble, { input, output } = {}) {
    if (input == null && output == null) return;
    const btn = el('button', {
        class: 'raw-toggle',
        type: 'button',
        title: 'Show raw input + output',
        'aria-label': 'Show raw input + output',
        onClick: () => showRawPopup({ input, output }),
    }, '{ }');
    bubble.appendChild(document.createTextNode(' '));
    bubble.appendChild(btn);
}

// Cheap deterministic templating for slash-skill results whose shape is small
// and unambiguous (single-key results, error envelopes, clock format). Returns
// the rendered string when a template matches, or `null` to fall through to
// the LLM-format pass. Saves a full LLM round-trip + the small-model verbosity
// for simple cases like `/calculator 5+5` → `{"result": 10}`.
function templateSimpleResult(cmd, raw) {
    let v;
    try { v = JSON.parse(raw); } catch { return null; }
    if (v == null || typeof v !== 'object' || Array.isArray(v)) return null;
    const keys = Object.keys(v);

    // Error envelope: every skill emits `{error: "..."}` on failure.
    if (keys.length === 1 && typeof v.error === 'string') {
        return `**Error:** ${v.error}`;
    }

    // /calculator → {result: number}
    if (cmd === 'calculator' && keys.length === 1 && typeof v.result === 'number') {
        // Render floats without trailing zeros: 8.0 → 8, 3.14 → 3.14.
        const n = Number.isInteger(v.result) ? String(v.result) : String(v.result);
        return `**${n}**`;
    }

    // /clock → {time: "ISO 8601", tz: "UTC"}
    if (cmd === 'clock' && typeof v.time === 'string') {
        // Prettify the ISO timestamp to "YYYY-MM-DD HH:MM:SS UTC" so users
        // don't have to mentally parse the T-separator.
        const m = v.time.match(/^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})/);
        const stamp = m ? `${m[1]} ${m[2]}` : v.time;
        const tz = typeof v.tz === 'string' ? v.tz : 'UTC';
        return `**${stamp} ${tz}**`;
    }

    // /web-fetch → {status, url, content_type?, body, truncated}
    // Known shape, so we template rather than feeding (potentially huge)
    // page bodies to a small LLM. Raw button still gives the full body.
    if (cmd === 'web-fetch' && typeof v.status === 'number' && typeof v.body === 'string') {
        const url = typeof v.url === 'string' ? v.url : '';
        const ct = typeof v.content_type === 'string' && v.content_type ? v.content_type : 'unknown';
        const trunc = v.truncated ? ' (truncated)' : '';
        const bytes = v.body.length;
        const head = `**${v.status}** from \`${url}\` — ${bytes} chars, ${ct}${trunc}.`;
        // Short bodies: inline a preview so the user sees the content immediately.
        if (bytes > 0 && bytes <= 400) {
            return `${head}\n\n\`\`\`\n${v.body}\n\`\`\``;
        }
        return head;
    }

    // Generic one-key string-value (e.g. some future `{summary: "..."}` shape).
    if (keys.length === 1 && typeof v[keys[0]] === 'string' && v[keys[0]].length <= 200) {
        return v[keys[0]];
    }

    return null;
}

// Asks the loaded LLM to summarize a slash-skill JSON result in plain language.
// Streams tokens into `bubbleText` (replacing whatever was there). On any
// failure (no model, network error, …) falls back to the raw output so the
// user still sees something. Returns when the stream completes.
async function formatSlashResultWithLLM({ cmd, args, raw, bubbleText }) {
    const modelId = selectedModelId();
    if (!modelId) {
        bubbleText.textContent = raw;
        return;
    }
    // Limit prompt size — huge fetched documents shouldn't blow up the LLM.
    const MAX = 4000;
    const slice = raw.length > MAX ? `${raw.slice(0, MAX)}\n…[truncated, ${raw.length} bytes total]` : raw;
    const prompt =
        `The user ran the slash-command \`/${cmd}${args ? ' ' + args : ''}\`. ` +
        `The skill returned this JSON output:\n\n\`\`\`json\n${slice}\n\`\`\`\n\n` +
        `Reply with a short, friendly answer in plain language that directly addresses ` +
        `what the user asked. Use 1–2 sentences. Do not mention "the JSON", "the skill", ` +
        `or "the output" — just state the result naturally.`;
    const thinker = startThinking(bubbleText, 'summarizing');
    let acc = '';
    try {
        const resp = await fetch('/b/agent/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                user_message: prompt,
                messages: [],
                model_id: modelId,
            }),
        });
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
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
                const lines = frame.split('\n');
                let event = '';
                let data = '';
                for (const line of lines) {
                    if (line.startsWith('event:')) event = line.slice(6).trim();
                    else if (line.startsWith('data:')) data += line.slice(5).replace(/^ /, '');
                }
                if (event === 'token') {
                    let p; try { p = JSON.parse(data); } catch { p = null; }
                    if (p?.delta) {
                        if (acc === '') thinker.stop();
                        acc += p.delta;
                        renderAssistantContent(bubbleText, acc);
                        scrollToBottom();
                    }
                }
            }
        }
    } catch (_e) {
        // fall through — show raw
    } finally {
        thinker.stop();
        if (!acc) {
            // LLM produced nothing — fall back to raw so user sees something.
            bubbleText.textContent = raw;
        }
    }
}

/**
 * When a slash skill returns an error, run a small LLM follow-up that takes
 * the user's original message + the skill error and emits a friendly
 * "did you mean…?" suggestion in plain language. Falls back to the raw error
 * envelope when no model is loaded or the LLM call fails.
 */
async function suggestCorrectionWithLLM({ cmd, args, error, userMessage, bubbleText }) {
    const modelId = selectedModelId();
    if (!modelId) {
        bubbleText.textContent = `Error: ${error}`;
        return;
    }
    const prompt =
        `The user typed: ${JSON.stringify(userMessage)}\n\n` +
        `That dispatched the \`/${cmd}\` skill, which failed with this error:\n\n` +
        `> ${error}\n\n` +
        `Write a one-sentence response in plain language explaining what likely went wrong, ` +
        `and (if possible) suggest a corrected version of the user's message. ` +
        `Do not echo the error verbatim and do not mention "the skill" or "the LLM" — ` +
        `just talk to the user like a helpful assistant.`;
    const thinker = startThinking(bubbleText, 'figuring');
    let acc = '';
    try {
        const resp = await fetch('/b/agent/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                user_message: prompt,
                messages: [],
                model_id: modelId,
            }),
        });
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
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
                const lines = frame.split('\n');
                let event = '';
                let data = '';
                for (const line of lines) {
                    if (line.startsWith('event:')) event = line.slice(6).trim();
                    else if (line.startsWith('data:')) data += line.slice(5).replace(/^ /, '');
                }
                if (event === 'token') {
                    let p; try { p = JSON.parse(data); } catch { p = null; }
                    if (p?.delta) {
                        if (acc === '') thinker.stop();
                        acc += p.delta;
                        renderAssistantContent(bubbleText, acc);
                        scrollToBottom();
                    }
                }
            }
        }
    } catch (_e) {
        // fall through — show raw error
    } finally {
        thinker.stop();
        if (!acc) {
            bubbleText.textContent = `Error: ${error}`;
        }
    }
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

// PR 6: slash-command autocomplete.
//
// Fetches `/b/agent/commands` once on page init (cached in localStorage
// for one hour) and wires an input listener on the composer textarea
// that:
//
//   - Pattern-matches `^/[A-Za-z0-9_-]*$` on the textarea value (no
//     whitespace allowed — `/imagine ` is already past the autocomplete
//     surface).
//   - Filters the command list by prefix, then by substring, on the
//     post-slash characters.
//   - Renders matches as a list above the composer; ArrowUp/Down
//     navigates highlight, Tab/Enter completes to `/<cmd> `, Escape
//     dismisses.
const SLASH_CACHE_KEY = 'gizza.commands.cache';
const SLASH_CACHE_TTL_MS = 60 * 60 * 1000; // 1h
const SLASH_RE = /^\/([A-Za-z0-9_-]*)$/;

let slashCommands = null;  // Array<{cmd, description}> or null until first fetch.
let slashDropdown = null;
let slashIndex = 0;
let slashFiltered = [];

function readSlashCache() {
    try {
        const raw = localStorage.getItem(SLASH_CACHE_KEY);
        if (!raw) return null;
        const { at, data } = JSON.parse(raw);
        if (!at || !Array.isArray(data)) return null;
        if (Date.now() - at > SLASH_CACHE_TTL_MS) return null;
        return data;
    } catch {
        return null;
    }
}

function writeSlashCache(data) {
    try {
        localStorage.setItem(SLASH_CACHE_KEY, JSON.stringify({ at: Date.now(), data }));
    } catch { /* quota / privacy mode — silently skip */ }
}

async function fetchSlashCommands(force = false) {
    if (!force) {
        const cached = readSlashCache();
        if (cached) {
            slashCommands = cached;
            return;
        }
    }
    try {
        const resp = await fetch('/b/agent/commands');
        if (!resp.ok) { slashCommands = []; return; }
        const data = await resp.json();
        if (!Array.isArray(data)) { slashCommands = []; return; }
        slashCommands = data;
        writeSlashCache(data);
    } catch { slashCommands = []; }
}

function ensureSlashDropdown() {
    if (slashDropdown) return slashDropdown;
    const composer = document.getElementById('composer');
    const input = document.getElementById('user-input');
    if (!composer || !input) return null;
    slashDropdown = el('div', {
        id: 'slash-autocomplete',
        role: 'listbox',
        'aria-label': 'Slash commands',
        hidden: '',
    });
    // Insert before the textarea so it stacks above visually via CSS.
    composer.insertBefore(slashDropdown, input);
    return slashDropdown;
}

function hideSlashDropdown() {
    if (!slashDropdown) return;
    slashDropdown.hidden = true;
    slashDropdown.replaceChildren();
    slashFiltered = [];
    slashIndex = 0;
}

function renderSlashDropdown(query) {
    if (!slashCommands || slashCommands.length === 0) {
        hideSlashDropdown();
        return;
    }
    const dropdown = ensureSlashDropdown();
    if (!dropdown) return;
    const lowered = query.toLowerCase();
    // Prefix matches first, substring matches after — both stable.
    const prefix = slashCommands.filter((c) => c.cmd.toLowerCase().startsWith(lowered));
    const substr = slashCommands.filter(
        (c) => !c.cmd.toLowerCase().startsWith(lowered)
            && c.cmd.toLowerCase().includes(lowered),
    );
    slashFiltered = [...prefix, ...substr];
    if (slashFiltered.length === 0) {
        hideSlashDropdown();
        return;
    }
    if (slashIndex >= slashFiltered.length) slashIndex = 0;
    dropdown.replaceChildren();
    slashFiltered.forEach((cmd, i) => {
        const item = el('button', {
            class: 'slash-item' + (i === slashIndex ? ' is-active' : ''),
            role: 'option',
            type: 'button',
            'aria-selected': i === slashIndex ? 'true' : 'false',
            'data-cmd': cmd.cmd,
        });
        item.appendChild(el('span', { class: 'slash-cmd' }, `/${cmd.cmd}`));
        if (cmd.description) {
            item.appendChild(el('span', { class: 'slash-desc' }, cmd.description));
        }
        item.addEventListener('mousedown', (e) => {
            // mousedown rather than click so we win the race against the
            // textarea's blur handler which would have hidden the dropdown.
            e.preventDefault();
            completeSlash(cmd.cmd);
        });
        dropdown.appendChild(item);
    });
    dropdown.hidden = false;
}

function completeSlash(cmd) {
    const input = document.getElementById('user-input');
    if (!input) return;
    input.value = `/${cmd} `;
    hideSlashDropdown();
    input.focus();
    // Move caret to end.
    const end = input.value.length;
    input.setSelectionRange(end, end);
}

function handleSlashInput() {
    const input = document.getElementById('user-input');
    if (!input) return;
    const match = SLASH_RE.exec(input.value);
    if (!match) {
        hideSlashDropdown();
        return;
    }
    if (!slashCommands) {
        // First slash before fetch completed — kick a fetch and bail; the
        // user is likely still typing, the next keystroke will re-render
        // once the cache is warm.
        fetchSlashCommands(true).then(() => {
            if (SLASH_RE.test(input.value)) handleSlashInput();
        });
        return;
    }
    renderSlashDropdown(match[1]);
}

function handleSlashKeydown(e) {
    if (!slashDropdown || slashDropdown.hidden || slashFiltered.length === 0) return;
    if (e.key === 'ArrowDown') {
        e.preventDefault();
        slashIndex = (slashIndex + 1) % slashFiltered.length;
        const input = document.getElementById('user-input');
        renderSlashDropdown(SLASH_RE.exec(input.value)?.[1] ?? '');
    } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        slashIndex = (slashIndex - 1 + slashFiltered.length) % slashFiltered.length;
        const input = document.getElementById('user-input');
        renderSlashDropdown(SLASH_RE.exec(input.value)?.[1] ?? '');
    } else if (e.key === 'Tab' || e.key === 'Enter') {
        // Enter would otherwise submit the form — intercept and complete.
        e.preventDefault();
        const chosen = slashFiltered[slashIndex];
        if (chosen) completeSlash(chosen.cmd);
    } else if (e.key === 'Escape') {
        e.preventDefault();
        hideSlashDropdown();
    }
}

// Wire up the autocomplete once the DOM is ready. fetchSlashCommands
// runs in the background; the input handler is responsive even before
// it resolves (slashCommands stays null and renderSlashDropdown bails).
(function initSlashAutocomplete() {
    const input = document.getElementById('user-input');
    if (!input) return;
    input.addEventListener('input', handleSlashInput);
    input.addEventListener('keydown', handleSlashKeydown);
    // Enter (without Shift) submits the chat form. Shift+Enter falls
    // through to the textarea's native newline behaviour. Slash-autocomplete
    // handler runs first and calls preventDefault when the dropdown is up,
    // so the `defaultPrevented` guard keeps this submit out of the way of
    // completion.
    input.addEventListener('keydown', (e) => {
        if (e.key !== 'Enter' || e.shiftKey) return;
        if (e.defaultPrevented) return;
        e.preventDefault();
        const form = document.getElementById('composer');
        form?.requestSubmit?.();
    });
    input.addEventListener('blur', () => {
        // Slight delay so a mousedown on the dropdown gets to fire first.
        setTimeout(hideSlashDropdown, 100);
    });
    fetchSlashCommands();
})();

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

// --- Picker overlay integration ---

// Start fetching the WebLLM module at page load so the first picker open
// doesn't pay ~5MB of CDN download cost. By the time the user clicks the
// brain icon, the promise has typically resolved and the await is a no-op.
const _webllmModulePromise = import('https://cdn.jsdelivr.net/npm/@mlc-ai/web-llm@0.2.74/+esm');

async function launchPicker() {
    const mod = await _webllmModulePromise;
    const prebuiltList = mod?.prebuiltAppConfig?.model_list || [];
    const result = await openPicker({
        prebuiltList,
        getCurrentModelId: getCurrentEngineModelId,
        loadEngine,
        unloadEngine,
        // Picker drives load/unload itself; reflect the result onto the chat
        // surface as the active model changes (incl. on unload → null).
        onActiveChange: (modelId) => {
            _loadedModelId = modelId;
            const send = $('send');
            if (send) send.disabled = !modelId;
            if (modelId) {
                localStorage.setItem(SELECTED_MODEL_KEY, modelId);
            }
            const btn = $('open-model-picker') || $('load-model');
            if (btn) {
                btn.disabled = false;
                btn.textContent = modelId ? 'Change model' : 'Load model';
            }
            if (modelId) clearLoadProgress();
            // Swap the bootstrap "I can't do much without a brain…" copy
            // for a friendlier ready hint. On unload (modelId === null) we
            // restore the cold-state copy so the chat surface explains
            // again why nothing's happening.
            const empty = document.querySelector('#messages .empty');
            if (empty) {
                if (modelId) {
                    empty.replaceChildren(document.createTextNode('Ready — ask anything below.'));
                } else if (empty.childElementCount === 0) {
                    // Only restore the prompt if we previously replaced it
                    // with a plain text node; if the original maud HTML is
                    // still there, leave it alone.
                    empty.replaceChildren(document.createTextNode("I can't do much without a brain — load one from the picker."));
                }
            }
        },
    });
    // After the picker closes, persist the final active model. If the user
    // closed the picker mid-load, `result` reflects the active model at close
    // time (which may not yet be the in-flight one); `onActiveChange` above
    // will fire when the load eventually completes.
    if (result?.model_id) {
        localStorage.setItem(SELECTED_MODEL_KEY, result.model_id);
    }
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
    // Show a cycling status while we wait for the first token / tool_result.
    const thinker = startThinking(assistantEl);
    // Slash-command bookkeeping so the tool_result handler can call the LLM
    // formatter with the original cmd + args.
    const slashCmd = text.match(/^\/(\S+)(?:\s+(.+))?$/);
    const slashFollowups = [];

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

        // Slash-command path. For each parked slash tool_result:
        //
        //  1. If the skill returned `{error: …}`, run an LLM correction
        //     follow-up — feed the original user message + the error back to
        //     the model and ask for a friendly "did you mean…?" suggestion.
        //     Beats dumping a raw stack trace at the user.
        //  2. Otherwise try the cheap deterministic template
        //     (calculator/clock/web-fetch/…). When it matches we render
        //     instantly and skip the LLM pass.
        //  3. Otherwise fall through to the LLM-format pass.
        //
        // Either way the `{ }` button gets appended with both `input` (what
        // the LLM dispatched to the skill) and `raw` (what the skill
        // returned), so users can flip between the two in the popup.
        for (const f of slashFollowups) {
            let parsed = null;
            try { parsed = JSON.parse(f.raw); } catch { /* not JSON */ }
            const skillError = parsed && typeof parsed === 'object' && !Array.isArray(parsed)
                && typeof parsed.error === 'string'
                ? parsed.error : null;

            if (skillError) {
                await suggestCorrectionWithLLM({
                    cmd: f.cmd, args: f.args, error: skillError, userMessage: text,
                    bubbleText: assistantEl,
                });
            } else {
                const templated = templateSimpleResult(f.cmd, f.raw);
                if (templated !== null) {
                    assistantEl.replaceChildren();
                    renderAssistantContent(assistantEl, templated);
                } else {
                    await formatSlashResultWithLLM({
                        cmd: f.cmd, args: f.args, raw: f.raw, bubbleText: assistantEl,
                    });
                }
            }
            appendRawButton(assistantEl.parentElement, { input: f.input, output: f.raw });
        }

        if (assistantText) {
            history.push({ role: 'assistant', content: assistantText });
        }
        roundTripCompleted = true;
    } catch (err) {
        thinker.stop();
        assistantEl.textContent = `(error: ${err.message})`;
    } finally {
        thinker.stop();
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
            if (assistantText === '') thinker.stop();
            assistantText += payload.delta;
            renderAssistantContent(assistantEl, assistantText);
            scrollToBottom();
        } else if (event === 'tool_call') {
            thinker.stop();
            const row = addToolRow(payload?.name ?? '?', payload?.arguments ?? '');
            toolRows.set(payload?.id ?? crypto.randomUUID(), row);
        } else if (event === 'tool_result') {
            thinker.stop();
            const row = toolRows.get(payload?.id);
            if (row) {
                updateToolRow(row, !payload?.error, payload?.result ?? payload?.error ?? '');
                renderToolAttachment(row, payload?.for_ui);
            } else {
                // Slash-command path: no preceding tool_call event because
                // the user-typed `/<cmd>` IS the call. Run the result
                // through the LLM for a plain-language summary and stash the
                // raw JSON behind a small `{ }` button. `for_ui` (images,
                // video) skips the formatter — the media speaks for itself.
                const summary = payload?.result ?? payload?.error ?? '';
                if (payload?.for_ui) {
                    renderToolAttachment(assistantEl, payload.for_ui);
                } else if (summary) {
                    // Defer the LLM formatting until the SSE stream is done
                    // so we don't race the original stream with a fresh
                    // /b/agent/chat. Park the work and run it after the loop.
                    // `input` is the params the agent dispatched to the skill
                    // (added in agent.rs alongside `result`) — surfaced in the
                    // `{ }` popup's Input tab so users can see what the LLM
                    // understood from their message.
                    slashFollowups.push({
                        cmd: slashCmd?.[1] || '?',
                        args: slashCmd?.[2] || '',
                        input: payload?.input ?? null,
                        raw: summary,
                    });
                }
                scrollToBottom();
            }
        } else if (event === 'confirm') {
            thinker.stop();
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
            thinker.stop();
            // Surface terminal errors and empty-stop states so the user
            // isn't left staring at a blank bubble. Token paths overwrite
            // assistantText already, so this only fires when nothing
            // streamed in.
            const reason = payload?.reason;
            const errText = payload?.error;
            if (reason === 'error') {
                if (errText && /no engine loaded/i.test(errText)) {
                    assistantText = '_No model loaded — pick one first._';
                    renderAssistantContent(assistantEl, assistantText);
                    launchPicker();
                } else {
                    assistantText = `_(agent error: ${errText || 'unknown'})_`;
                    renderAssistantContent(assistantEl, assistantText);
                }
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
