//! gizza-ai model picker — groups, fetches, renders. See spec at
//! docs/superpowers/specs/2026-05-08-gizza-ai-model-picker-redesign-design.md.

// The maud-rendered chat page (src/blocks/ui.rs) doesn't yet <link> to
// /model-picker.css — the Rust-side cleanup is deferred behind solobase build.
// Inject the stylesheet on module load so the picker styling lands without a
// rebuild. Once ui.rs ships the link, the idempotent guard makes this a no-op.
if (typeof document !== 'undefined' && !document.querySelector('link[href="/model-picker.css"]')) {
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = '/model-picker.css';
    document.head.appendChild(link);
}

const FAMILY_MAP = {
    Llama: 'Meta',
    Qwen: 'Alibaba',
    Phi: 'Microsoft',
    Hermes: 'NousResearch',
    gemma: 'Google',
    Mistral: 'Mistral AI',
    SmolLM: 'HuggingFaceTB',
    stablelm: 'Stability AI',
    RedPajama: 'Together',
    TinyLlama: 'TinyLlama',
    OpenHermes: 'NousResearch',
    NeuralHermes: 'NousResearch',
    WizardMath: 'WizardLM',
    snowflake: 'Snowflake',
};

const QUALITY_LABELS = {
    'q0f16': { label: 'High quality', sublabel: 'q0f16' },
    'q0f32': { label: 'High quality', sublabel: 'q0f32' },
    'q4f16_1': { label: 'Balanced', sublabel: 'q4f16' },
    'q4f32_1': { label: 'Standard', sublabel: 'q4f32' },
    'q3f16_1': { label: 'Smallest', sublabel: 'q3f16' },
};

const QUALITY_SORT_ORDER = ['q0f32', 'q0f16', 'q4f32_1', 'q4f16_1', 'q3f16_1'];

function detectFamily(baseId) {
    for (const prefix of Object.keys(FAMILY_MAP)) {
        if (baseId.toLowerCase().startsWith(prefix.toLowerCase())) return FAMILY_MAP[prefix];
    }
    return 'Other';
}

function paramsLabel(baseId) {
    const m = baseId.match(/(\d+(?:\.\d+)?)[Bb](?![a-z])/);
    return m ? `${m[1]}B params` : null;
}

function stripVariantSuffix(modelId) {
    // <base>-<quant>-MLC[-<ctx>][-<batch>] → <base>
    // -<ctx>   is `-Nk` (optionally `_csNk`) on chat models
    // -<batch> is `-bN` on embedding models (e.g. snowflake-arctic-embed)
    return modelId.replace(/-q\d+f\d+(_\d+)?-MLC(-\d+k(_cs\d+k)?)?(-b\d+)?$/, '');
}

function extractQuant(modelId) {
    const m = modelId.match(/-q(\d+)f(\d+)(_(\d+))?-MLC/);
    if (!m) return 'unknown';
    return `q${m[1]}f${m[2]}${m[3] || ''}`;
}

export function groupModels(prebuiltList) {
    const byBase = new Map();
    for (const entry of prebuiltList) {
        if (!entry?.model_id) continue;
        const baseId = stripVariantSuffix(entry.model_id);
        if (!byBase.has(baseId)) {
            byBase.set(baseId, {
                base_id: baseId,
                family: detectFamily(baseId),
                params_label: paramsLabel(baseId),
                has_vision: /vision/i.test(baseId),
                hf_url: entry.model || null,
                ctx: entry.overrides?.context_window_size || null,
                variants: [],
            });
        }
        const group = byBase.get(baseId);
        const quant = extractQuant(entry.model_id);
        // The MLC list often contains both the full-context and a `-1k` clipped
        // variant of the same quant (e.g. `…-q4f16_1-MLC` and `…-q4f16_1-MLC-1k`).
        // Both strip to the same base_id, which would otherwise duplicate the
        // quality button. Keep the first occurrence — that's the larger-context
        // model_id which appears earlier in the prebuilt list.
        if (group.variants.some((v) => v.quant === quant)) {
            continue;
        }
        const labelDef = QUALITY_LABELS[quant] || { label: quant, sublabel: quant };
        group.variants.push({
            id: entry.model_id,
            quant,
            label: labelDef.label,
            sublabel: labelDef.sublabel,
            vram_mb: entry.vram_required_MB || null,
            hf_url: entry.model || null,
        });
        if (!group.ctx && entry.overrides?.context_window_size) group.ctx = entry.overrides.context_window_size;
    }
    // Sort variants within each group by quality tier
    for (const group of byBase.values()) {
        group.variants.sort((a, b) => {
            const ai = QUALITY_SORT_ORDER.indexOf(a.quant);
            const bi = QUALITY_SORT_ORDER.indexOf(b.quant);
            const ax = ai === -1 ? QUALITY_SORT_ORDER.length : ai;
            const bx = bi === -1 ? QUALITY_SORT_ORDER.length : bi;
            return ax - bx;
        });
    }
    return Array.from(byBase.values());
}

const HF_CACHE_KEY = 'gizza:hf-popularity-cache';
const HF_CACHE_TTL_MS = 24 * 60 * 60 * 1000;
const HF_API_URL = 'https://huggingface.co/api/models?author=mlc-ai&limit=300&full=false';

let _inFlightRefresh = null;

export function _resetPopularityCache() {
    _inFlightRefresh = null;
}

function readCache(localStorage) {
    try {
        const raw = localStorage.getItem(HF_CACHE_KEY);
        if (!raw) return null;
        const parsed = JSON.parse(raw);
        if (typeof parsed?.fetched_ms !== 'number' || typeof parsed?.data !== 'object') return null;
        return parsed;
    } catch (_e) {
        return null;
    }
}

function writeCache(localStorage, fetched_ms, data) {
    try {
        localStorage.setItem(HF_CACHE_KEY, JSON.stringify({ fetched_ms, data }));
    } catch (_e) {
        // localStorage full or disabled — ignore, popularity will refetch next time.
    }
}

async function fetchAndStore(localStorage, fetchFn, now) {
    try {
        const resp = await fetchFn(HF_API_URL);
        if (!resp?.ok) return null;
        const list = await resp.json();
        const data = {};
        for (const repo of list) {
            const id = typeof repo?.id === 'string' ? repo.id : null;
            if (!id || !id.startsWith('mlc-ai/')) continue;
            const modelId = id.slice('mlc-ai/'.length);
            data[modelId] = {
                downloads: typeof repo.downloads === 'number' ? repo.downloads : 0,
                likes: typeof repo.likes === 'number' ? repo.likes : 0,
            };
        }
        const ts = now();
        writeCache(localStorage, ts, data);
        return data;
    } catch (_e) {
        return null;
    }
}

export async function fetchHfPopularity({
    localStorage = globalThis.localStorage,
    fetch = globalThis.fetch,
    now = () => Date.now(),
} = {}) {
    const cached = readCache(localStorage);
    const fresh = cached && (now() - cached.fetched_ms) < HF_CACHE_TTL_MS;
    if (cached && fresh) return cached.data;
    if (cached && !fresh) {
        // Stale: return cached now, refresh in background.
        if (!_inFlightRefresh) {
            _inFlightRefresh = fetchAndStore(localStorage, fetch, now).finally(() => {
                _inFlightRefresh = null;
            });
        }
        return cached.data;
    }
    // Cache miss: fetch synchronously, fall back to empty.
    const data = await fetchAndStore(localStorage, fetch, now);
    return data || {};
}

/**
 * Walks WebLLM's OPFS cache and returns the set of base_ids that have at least
 * one variant cached, plus the currently-loaded model_id (if any).
 *
 * WebLLM (0.2.74) stores its model shards under
 *   `webllm/<model_id>/...`
 * inside the origin's OPFS. We open the `webllm/` directory and list its
 * top-level entries — each child is a `model_id`. We do NOT recurse; presence
 * of the directory is sufficient to mark a base as cached.
 *
 * Failures (no OPFS, no `webllm/` directory yet, permission denied) return an
 * empty Set so the picker still works on the cold path.
 */
export async function getCachedAndActive(baseModels, currentModelId = null) {
    const cached = new Set();
    try {
        const root = await navigator.storage?.getDirectory?.();
        if (!root) return { cached, active: currentModelId };
        let webllmDir;
        try {
            webllmDir = await root.getDirectoryHandle('webllm');
        } catch (_e) {
            return { cached, active: currentModelId };
        }
        const cachedIds = new Set();
        for await (const [name, handle] of webllmDir.entries()) {
            if (handle.kind === 'directory') cachedIds.add(name);
        }
        for (const group of baseModels) {
            for (const variant of group.variants) {
                if (cachedIds.has(variant.id)) {
                    cached.add(group.base_id);
                    break;
                }
            }
        }
    } catch (_e) {
        // Any unexpected failure → treat as nothing cached.
    }
    return { cached, active: currentModelId };
}

/**
 * Deletes every WebLLM-cached variant of `group` from OPFS.
 *
 * Iterates `group.variants` and calls `webllmDir.removeEntry(variant.id, { recursive: true })`
 * on each. Each variant is removed independently — a partial cache cleans up cleanly
 * because per-variant failures are caught and ignored. Failures (no OPFS, no `webllm/`
 * directory, permission denied, missing entry) are silent — same defensive posture as
 * `getCachedAndActive`.
 *
 * Caller is responsible for re-running `getCachedAndActive` after this resolves to
 * refresh UI state.
 */
export async function deleteCachedModel(group) {
    try {
        const root = await navigator.storage?.getDirectory?.();
        if (!root) return;
        let webllmDir;
        try {
            webllmDir = await root.getDirectoryHandle('webllm');
        } catch (_e) {
            return;
        }
        for (const variant of group.variants) {
            try {
                await webllmDir.removeEntry(variant.id, { recursive: true });
            } catch (_e) {
                // missing or permission-denied — fall through, try the next variant
            }
        }
    } catch (_e) {
        // any unexpected failure → noop
    }
}

const SIZE_TIERS = [
    { id: 'small', label: 'Small (<2 GB)', max_mb: 2048 },
    { id: 'medium', label: 'Medium (2–5 GB)', min_mb: 2048, max_mb: 5120 },
    { id: 'large', label: 'Large (5+ GB)', min_mb: 5120 },
];

/**
 * Curated brain tiers shown on the picker landing.
 *
 * Each tier carries a `f16` and `f32` variant. `q4f16_1` quantizations are
 * smaller but require the WebGPU `shader-f16` feature — older GPUs and many
 * Linux drivers don't expose it, so we probe at open time
 * ([`probeShaderF16`]) and resolve to the f32 fallback when missing. Picks
 * are all Qwen2.5 for ladder coherence — same family, just more brain.
 * Medium is flagged `recommended` so the empty-state defaults to it.
 */
const BRAIN_TIERS = [
    {
        id: 'small',
        label: 'Small brain',
        emoji: '🧠',
        variants: {
            f16: { id: 'Qwen2.5-0.5B-Instruct-q4f16_1-MLC', size_gb: 0.9 },
            f32: { id: 'Qwen2.5-0.5B-Instruct-q4f32_1-MLC', size_gb: 1.0 },
        },
        tagline: 'Fastest. Quick chats and simple slash commands.',
    },
    {
        id: 'medium',
        label: 'Medium brain',
        emoji: '🧠',
        variants: {
            f16: { id: 'Qwen2.5-1.5B-Instruct-q4f16_1-MLC', size_gb: 1.6 },
            f32: { id: 'Qwen2.5-1.5B-Instruct-q4f32_1-MLC', size_gb: 1.9 },
        },
        tagline: 'Balanced. Reliably understands slash commands.',
        recommended: true,
    },
    {
        id: 'big',
        label: 'Big brain',
        emoji: '🧠',
        variants: {
            f16: { id: 'Qwen2.5-3B-Instruct-q4f16_1-MLC', size_gb: 2.4 },
            f32: { id: 'Qwen2.5-3B-Instruct-q4f32_1-MLC', size_gb: 2.9 },
        },
        tagline: 'Smartest. Better reasoning, longer to load.',
    },
];

/**
 * Probe the user's WebGPU adapter for the `shader-f16` feature. q4f16
 * quantizations emit `enable f16;` at the top of their compute shader and the
 * device rejects compilation if the feature is absent. We default to `false`
 * on any error so the picker always lands on a working variant.
 */
export async function probeShaderF16() {
    try {
        if (typeof navigator === 'undefined' || !navigator.gpu) return false;
        const adapter = await navigator.gpu.requestAdapter();
        return !!adapter?.features?.has('shader-f16');
    } catch (_e) {
        return false;
    }
}

/** Resolve a tier to its concrete `(model_id, size_gb)` based on the f16 probe. */
function resolveTierVariant(tier, f16Supported) {
    const v = f16Supported && tier.variants.f16 ? tier.variants.f16 : tier.variants.f32;
    return { ...tier, model_id: v.id, size_gb: v.size_gb };
}

/**
 * Locate the (group, variant) tuple for a tier's `model_id` in the grouped
 * model list. Returns `null` when WebLLM has dropped the exact variant (rare
 * but possible across versions). Caller can use that to grey out the card
 * with a "model not available in this WebLLM version" message.
 */
function resolveTierGroupVariant(groups, modelId) {
    for (const g of groups) {
        const v = g.variants.find((x) => x.id === modelId);
        if (v) return { group: g, variant: v };
    }
    return null;
}

const FAMILY_CHIP_OPTIONS = ['Llama', 'Qwen', 'Phi', 'Hermes', 'Gemma', 'Mistral', 'Other'];

const SORT_OPTIONS = [
    { value: 'downloaded-popular', label: 'Favorites, downloaded, then popular' },
    { value: 'popular', label: 'Most popular' },
    { value: 'smallest', label: 'Smallest first' },
    { value: 'largest', label: 'Largest first' },
    { value: 'az', label: 'Model A–Z' },
    { value: 'za', label: 'Model Z–A' },
    { value: 'provider-az', label: 'Provider A–Z' },
    { value: 'provider-za', label: 'Provider Z–A' },
];

function smallestVramMb(group) {
    const mbs = group.variants.map((v) => v.vram_mb).filter((x) => typeof x === 'number');
    return mbs.length ? Math.min(...mbs) : 0;
}

function el(tag, attrs = {}, children = []) {
    const node = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
        if (k === 'class') node.className = v;
        else if (k === 'onClick') node.addEventListener('click', v);
        else if (k === 'onInput') node.addEventListener('input', v);
        else if (k === 'onChange') node.addEventListener('change', v);
        else if (v === true) node.setAttribute(k, '');
        else if (v !== false && v != null) node.setAttribute(k, v);
    }
    for (const c of children) {
        if (c == null) continue;
        node.appendChild(typeof c === 'string' ? document.createTextNode(c) : c);
    }
    return node;
}

function formatBytes(mb) {
    if (!mb) return '—';
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
}

function formatDownloads(n) {
    if (n == null) return null;
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M ↓`;
    if (n >= 1000) return `${Math.round(n / 1000)}k ↓`;
    return `${n} ↓`;
}

/**
 * Does a quant string carry an f16 payload that the WebGPU shader will need
 * the `shader-f16` extension for? Any `…f16…` substring matches (covers
 * `q0f16`, `q4f16_1`, `q3f16_1`, etc.); `q*f32*` quants compile without the
 * extension and are universally supported.
 */
function requiresShaderF16(quant) {
    return /f16/i.test(quant || '');
}

/**
 * Pick a sensible default variant for a row in the custom-table view. When
 * `shader-f16` is missing we prefer the first non-f16 variant so the row
 * lands on a loadable quantization; falls back to the middle variant (which
 * the UI will mark disabled) if every variant needs f16.
 */
function pickInitialVariant(group, f16Supported) {
    const middle = group.variants[Math.floor(group.variants.length / 2)] || group.variants[0];
    if (f16Supported) return middle;
    return group.variants.find((v) => !requiresShaderF16(v.quant)) || middle;
}

function renderTableRow(group, ctx) {
    const f16Supported = ctx.f16Supported !== false;
    const initialVariant = ctx.selection?.base_id === group.base_id
        ? ctx.selection.variant
        : pickInitialVariant(group, f16Supported);
    const isCached = ctx.cached.has(group.base_id);
    const isActive = group.variants.some((v) => v.id === ctx.active);
    const loadingVariantId = ctx.loading?.variantId || null;
    const loadingInGroup = loadingVariantId
        && group.variants.some((v) => v.id === loadingVariantId);
    const anyLoad = loadingVariantId != null;
    const initialBlockedByF16 = !f16Supported && requiresShaderF16(initialVariant?.quant);

    const tr = el('tr', {
        class: ['mp-row',
            isActive ? 'is-active' : '',
            isCached && !isActive ? 'is-cached' : '',
            loadingInGroup ? 'is-loading' : '',
        ].filter(Boolean).join(' '),
        'data-base-id': group.base_id,
    });

    // Model cell — favorite star + name (+ HF link on sub line).
    const isFav = ctx.favorites?.has(group.base_id);
    const favoriteBtn = el('button', {
        type: 'button',
        class: `mp-star ${isFav ? 'active' : ''}`,
        'aria-label': isFav ? `Unfavorite ${group.base_id}` : `Favorite ${group.base_id}`,
        title: isFav ? 'Unfavorite' : 'Favorite',
        onClick: (e) => {
            e.stopPropagation();
            ctx.onFavoriteToggle?.(group.base_id);
        },
    }, [isFav ? '★' : '☆']);
    const trashBtn = isCached && !isActive && !loadingInGroup ? el('button', {
        type: 'button',
        class: 'mp-trash',
        'aria-label': `Delete cached ${group.base_id}`,
        title: 'Delete cached files',
        onClick: (e) => {
            e.stopPropagation();
            ctx.onDeleteCached?.(group);
        },
    }, ['🗑']) : null;
    const modelCell = el('td', { class: 'mp-cell-model' }, [
        el('div', { class: 'mp-cell-model-name' }, [
            favoriteBtn,
            el('span', { class: 'mp-cell-model-title' }, [group.base_id]),
            trashBtn,
        ]),
        el('div', { class: 'mp-cell-model-sub' }, [
            [group.family, group.params_label].filter(Boolean).join(' · '),
            group.hf_url ? el('a', {
                class: 'mp-cell-hf-link',
                href: group.hf_url,
                target: '_blank',
                rel: 'noopener',
                title: 'Open on HuggingFace',
                onClick: (e) => e.stopPropagation(),
            }, [' · HF ↗']) : null,
        ]),
    ]);
    tr.appendChild(modelCell);

    // Provider.
    tr.appendChild(el('td', { class: 'mp-cell-provider' }, [group.family || '—']));

    // Variant dropdown — disabled while this row is loading. Inside the
    // dropdown, f16 quantizations are individually disabled when the user's
    // GPU lacks `shader-f16`.
    const variantCell = el('td', { class: 'mp-cell-variant' });
    const qualityPicker = renderQualityPicker(group, initialVariant, f16Supported);
    if (loadingInGroup) {
        qualityPicker.querySelector('select')?.setAttribute('disabled', '');
    }
    variantCell.appendChild(qualityPicker);
    tr.appendChild(variantCell);

    // Size — updates when variant changes.
    tr.appendChild(el('td', { class: 'mp-cell-size' }, [
        el('strong', { class: 'mp-size-value' }, [formatBytes(initialVariant?.vram_mb)]),
    ]));

    // Capabilities.
    const caps = el('td', { class: 'mp-cell-caps' });
    if (group.ctx) caps.appendChild(el('span', { class: 'mp-badge ctx' }, [`${Math.round(group.ctx / 1024)}k ctx`]));
    tr.appendChild(caps);

    // Status — also hosts the per-row progress + error display.
    let statusCell;
    if (loadingInGroup) {
        const pct = ctx.loading?.percent;
        statusCell = el('td', { class: 'mp-cell-status is-loading' }, [
            el('div', { class: 'mp-load-progress' }, [
                el('div', {
                    class: 'mp-load-progress-bar',
                    style: typeof pct === 'number' ? `width: ${Math.max(0, Math.min(100, pct))}%` : 'width: 0%',
                }),
            ]),
            el('div', { class: 'mp-load-progress-text' }, [
                ctx.loading?.text || 'Starting…',
            ]),
        ]);
    } else if (ctx.errorForBase === group.base_id && ctx.errorText) {
        statusCell = el('td', { class: 'mp-cell-status is-error', title: ctx.errorText }, [
            el('span', { class: 'mp-status-error' }, [`! ${ctx.errorText.slice(0, 60)}`]),
        ]);
    } else if (isActive) {
        statusCell = el('td', { class: 'mp-cell-status is-loaded' }, ['✓ Loaded']);
    } else if (isCached) {
        statusCell = el('td', { class: 'mp-cell-status is-cached' }, ['✓ Cached']);
    } else {
        statusCell = el('td', { class: 'mp-cell-status is-empty' }, ['—']);
    }
    tr.appendChild(statusCell);

    // Actions — single button. Three states:
    //   - this row is loading → "Loading…" (disabled)
    //   - this row is the active engine → "Unload"
    //   - otherwise → "Load" (disabled when another row is loading)
    const actionCell = el('td', { class: 'mp-cell-actions' });
    let btn;
    if (loadingInGroup) {
        btn = el('button', {
            class: 'mp-action-btn mp-load-btn primary',
            type: 'button',
            disabled: '',
        }, ['Loading…']);
    } else if (isActive) {
        btn = el('button', {
            class: 'mp-action-btn mp-unload-btn',
            type: 'button',
            disabled: anyLoad ? '' : undefined,
            title: 'Release GPU memory (keeps cache)',
        }, ['Unload']);
    } else {
        const disableForF16 = initialBlockedByF16;
        let btnTitle;
        if (anyLoad) btnTitle = 'Another model is loading…';
        else if (disableForF16) btnTitle = 'shader-f16 not supported by your GPU — pick a different variant';
        else btnTitle = isCached ? 'Load into GPU' : 'Download & load';
        btn = el('button', {
            class: 'mp-action-btn mp-load-btn primary',
            type: 'button',
            disabled: anyLoad || disableForF16 ? '' : undefined,
            title: btnTitle,
        }, ['Load']);
    }
    actionCell.appendChild(btn);
    tr.appendChild(actionCell);

    return tr;
}

function renderQualityPicker(group, initialVariant, f16Supported = true) {
    // Single dropdown replaces the dense variant-pill row. Each option
    // includes the quality label + its quantization sublabel so users can
    // still tell variants apart without four side-by-side buttons. f16
    // variants are marked disabled (with a hint) when the user's WebGPU
    // adapter lacks the `shader-f16` feature — the shader fails to compile
    // otherwise.
    const wrap = el('div', { class: 'mp-quality' });
    const select = el('select', {
        class: 'mp-quality-select',
        'aria-label': 'Variant',
        onClick: (e) => e.stopPropagation(),
    });
    for (const v of group.variants) {
        const blocked = !f16Supported && requiresShaderF16(v.quant);
        const label = blocked
            ? `${v.label} (${v.sublabel}) — needs shader-f16`
            : `${v.label} (${v.sublabel})`;
        const opt = el('option', {
            value: v.id,
            'data-variant-id': v.id,
            'data-needs-f16': blocked ? 'true' : undefined,
            disabled: blocked ? '' : undefined,
            selected: v.id === initialVariant?.id ? '' : undefined,
        }, [label]);
        select.appendChild(opt);
    }
    wrap.appendChild(select);
    return wrap;
}

/**
 * Tier-card landing view: three big "brain" cards + a "Custom brain" panel
 * (the full table) gated behind a link. Reuses the same `ctx.loading` /
 * `ctx.active` state and the same `performLoad` / `performUnload` handlers
 * the table view drives, so the load lifecycle is identical from either path.
 *
 * `ctx.onLoadModelId(modelId)` and `ctx.onUnload()` are the entry points;
 * `ctx.onSwitchView('custom')` flips the picker to the full table.
 */
function renderTierCards(ctx) {
    const wrap = el('div', { class: 'mp-tiers' });

    // Intro line — sets the brain-metaphor expectation.
    wrap.appendChild(el('p', { class: 'mp-tiers-intro' }, [
        'Pick a brain to load into your browser. Bigger = smarter + slower download.',
    ]));

    const grid = el('div', { class: 'mp-tier-grid' });

    // `ctx.tiers` is the f16/f32-resolved list from openPicker; fall back to
    // raw BRAIN_TIERS with the f32 variant in case someone calls
    // renderTierCards in isolation (e.g. tests).
    const tiers = ctx.tiers
        || BRAIN_TIERS.map((t) => resolveTierVariant(t, false));

    for (const tier of tiers) {
        const resolved = resolveTierGroupVariant(ctx.groups, tier.model_id);
        const available = !!resolved;
        const isActive = ctx.active === tier.model_id;
        const isCached = available && ctx.cached.has(resolved.group.base_id);
        const isLoading = ctx.loading?.variantId === tier.model_id;
        const anyLoad = ctx.loading?.variantId != null;

        const cardClasses = ['mp-tier-card'];
        if (tier.recommended) cardClasses.push('is-recommended');
        if (isActive) cardClasses.push('is-active');
        if (isLoading) cardClasses.push('is-loading');
        if (!available) cardClasses.push('is-unavailable');

        const card = el('div', {
            class: cardClasses.join(' '),
            'data-tier-id': tier.id,
            'data-model-id': tier.model_id,
        });

        // Head: emoji + label + (optional) recommended badge.
        const head = el('div', { class: 'mp-tier-head' }, [
            el('span', { class: 'mp-tier-emoji' }, [tier.emoji]),
            el('div', { class: 'mp-tier-titles' }, [
                el('h3', { class: 'mp-tier-label' }, [tier.label]),
                el('div', { class: 'mp-tier-sub' }, [`${tier.size_gb} GB · ${tier.model_id.split('-q')[0]}`]),
            ]),
        ]);
        if (tier.recommended) {
            head.appendChild(el('span', { class: 'mp-tier-badge' }, ['Recommended']));
        }
        card.appendChild(head);

        card.appendChild(el('p', { class: 'mp-tier-tagline' }, [tier.tagline]));

        // Progress (only while this tier is loading).
        if (isLoading) {
            const pct = ctx.loading?.percent;
            card.appendChild(el('div', { class: 'mp-tier-progress' }, [
                el('div', {
                    class: 'mp-tier-progress-bar',
                    style: typeof pct === 'number' ? `width: ${Math.max(0, Math.min(100, pct))}%` : 'width: 0%',
                }),
            ]));
            card.appendChild(el('div', { class: 'mp-tier-progress-text' }, [ctx.loading?.text || 'Starting…']));
        }

        // Status — only render when there's something meaningful to say. An
        // empty cold-state card just shows the Load button + no status text.
        let status = null;
        if (isActive) {
            status = el('div', { class: 'mp-tier-status is-loaded' }, ['✓ Loaded']);
        } else if (isCached) {
            status = el('div', { class: 'mp-tier-status is-cached' }, ['✓ Cached']);
        } else if (!available) {
            status = el('div', { class: 'mp-tier-status is-unavailable' }, ['Not in this version']);
        }
        // While loading, the progress bar + text above the foot communicate
        // the live state — no extra label needed in the foot row.

        // Foot needs to render a placeholder element when status is null so
        // the button stays right-aligned via space-between.
        const statusSlot = status || el('div', { class: 'mp-tier-status-placeholder' });

        let btn;
        if (isLoading) {
            btn = el('button', { class: 'mp-tier-btn primary', type: 'button', disabled: '' }, ['Loading…']);
        } else if (isActive) {
            btn = el('button', {
                class: 'mp-tier-btn mp-tier-unload',
                type: 'button',
                disabled: anyLoad ? '' : undefined,
                title: 'Release GPU memory (keeps cached files)',
                onClick: () => ctx.onUnload?.(),
            }, ['Unload']);
        } else if (!available) {
            btn = el('button', { class: 'mp-tier-btn primary', type: 'button', disabled: '' }, ['Unavailable']);
        } else {
            btn = el('button', {
                class: 'mp-tier-btn primary',
                type: 'button',
                disabled: anyLoad ? '' : undefined,
                onClick: () => ctx.onLoadModelId?.(tier.model_id),
            }, ['Load brain']);
        }

        card.appendChild(el('div', { class: 'mp-tier-foot' }, [statusSlot, btn]));
        grid.appendChild(card);
    }

    wrap.appendChild(grid);

    // "Custom brain" escape hatch — drops into the full table view.
    wrap.appendChild(el('div', { class: 'mp-tiers-footer' }, [
        el('button', {
            class: 'mp-tiers-more',
            type: 'button',
            onClick: () => ctx.onSwitchView?.('custom'),
        }, ['Custom brain — browse all models →']),
    ]));

    return wrap;
}

export function renderPickerDom(ctx) {
    // ctx: { groups, popularity, cached, active, selection, filters, view,
    //        onClose, onFiltersChange, onSwitchView, onLoadModelId, onUnload }
    const dialog = el('dialog', { id: 'model-picker' });
    const view = ctx.view === 'custom' ? 'custom' : 'tiers';

    // Header
    const headerChildren = [
        el('h2', {}, [view === 'custom' ? 'Choose a model' : 'Pick a brain']),
        el('button', { class: 'mp-close', 'aria-label': 'Close', onClick: ctx.onClose }, ['✕']),
    ];
    if (view === 'custom') {
        // Show a back-to-tiers affordance only in the full-table view.
        headerChildren.splice(1, 0, el('button', {
            class: 'mp-back',
            type: 'button',
            'aria-label': 'Back to brain tiers',
            onClick: () => ctx.onSwitchView?.('tiers'),
        }, ['← Back']));
    }
    const header = el('div', { class: 'mp-header' }, headerChildren);

    // Tier-card landing: short-circuit before building filters + table.
    if (view === 'tiers') {
        dialog.appendChild(header);
        dialog.appendChild(renderTierCards(ctx));
        return { dialog, grid: null, thead: null, summaryEl: null, loadBtn: null, filtersEl: null, view };
    }

    // Filter row
    const filtersEl = el('div', { class: 'mp-filters' }, [
        el('input', {
            class: 'mp-search',
            type: 'search',
            placeholder: 'Search models…',
            'aria-label': 'Search models',
            onInput: (e) => ctx.onFiltersChange({ ...ctx.filters, search: e.target.value }),
        }),
        el('select', {
            class: 'mp-filter-select',
            'aria-label': 'Size filter',
            onChange: (e) => {
                const v = e.target.value;
                ctx.onFiltersChange({ ...ctx.filters, sizes: v ? [v] : [] });
            },
        }, [
            (() => {
                const o = el('option', { value: '' }, ['All sizes']);
                if (ctx.filters.sizes.length === 0) o.selected = true;
                return o;
            })(),
            ...SIZE_TIERS.map((tier) => {
                const o = el('option', { value: tier.id }, [tier.label]);
                if (ctx.filters.sizes[0] === tier.id) o.selected = true;
                return o;
            }),
        ]),
        el('select', {
            class: 'mp-filter-select',
            'aria-label': 'Provider filter',
            onChange: (e) => {
                const v = e.target.value;
                ctx.onFiltersChange({ ...ctx.filters, families: v ? [v] : [] });
            },
        }, [
            (() => {
                const o = el('option', { value: '' }, ['All providers']);
                if (ctx.filters.families.length === 0) o.selected = true;
                return o;
            })(),
            ...FAMILY_CHIP_OPTIONS.map((fam) => {
                const o = el('option', { value: fam }, [fam]);
                if (ctx.filters.families[0] === fam) o.selected = true;
                return o;
            }),
        ]),
        el('label', { class: 'mp-toggle' }, [
            el('input', {
                type: 'checkbox',
                checked: ctx.filters.visionOnly ? true : false,
                onChange: (e) => ctx.onFiltersChange({ ...ctx.filters, visionOnly: e.target.checked }),
            }),
            'Vision-capable',
        ]),
        el('label', { class: 'mp-toggle' }, [
            el('input', {
                type: 'checkbox',
                checked: ctx.filters.favoritesOnly ? true : false,
                onChange: (e) => ctx.onFiltersChange({ ...ctx.filters, favoritesOnly: e.target.checked }),
            }),
            '★ Favorites only',
        ]),
        el('select', {
            class: 'mp-sort',
            onChange: (e) => ctx.onFiltersChange({ ...ctx.filters, sort: e.target.value }),
        }, SORT_OPTIONS.map((opt) => {
            const o = el('option', { value: opt.value }, [opt.label]);
            if (opt.value === ctx.filters.sort) o.selected = true;
            return o;
        })),
        el('div', { class: 'mp-result-count', 'data-result-count': true }, []),
    ]);

    // Table — clickable headers toggle ascending/descending sort for that
    // column. The Sort dropdown (in the filter row) still works as the
    // canonical control; clicking a header just swaps `filters.sort` to the
    // matching value.
    const sortHeader = (label, asc, desc) => {
        const isActive = ctx.filters.sort === asc || ctx.filters.sort === desc;
        const indicator = ctx.filters.sort === asc ? ' ▲'
            : ctx.filters.sort === desc ? ' ▼'
            : '';
        const th = el('th', {
            class: `mp-th-sortable ${isActive ? 'is-active' : ''}`,
            role: 'button',
            tabindex: '0',
            onClick: () => {
                const next = ctx.filters.sort === asc ? desc : asc;
                ctx.onFiltersChange({ ...ctx.filters, sort: next });
            },
        }, [label + indicator]);
        return th;
    };
    const thead = el('thead', {}, [
        el('tr', {}, [
            sortHeader('Model', 'az', 'za'),
            sortHeader('Provider', 'provider-az', 'provider-za'),
            el('th', { class: 'mp-th-variant' }, ['Variant']),
            sortHeader('Size', 'smallest', 'largest'),
            el('th', { class: 'mp-th-caps' }, ['Capabilities']),
            el('th', { class: 'mp-th-status' }, ['Status']),
            el('th', { class: 'mp-th-actions' }, ['Actions']),
        ]),
    ]);
    const tbody = el('tbody', { class: 'mp-tbody' });
    const table = el('table', { class: 'mp-table' }, [thead, tbody]);
    const tableWrap = el('div', { class: 'mp-table-wrap' }, [table]);

    // Footer dropped — per-row Download/Load buttons replace the global one,
    // and the header's ✕ provides the cancel path.

    dialog.appendChild(header);
    // shader-f16 warning banner — surfaces the GPU limitation up-front so the
    // user understands why some Variant dropdown rows look greyed out.
    if (ctx.f16Supported === false) {
        dialog.appendChild(el('div', { class: 'mp-f16-banner' }, [
            el('span', { class: 'mp-f16-banner-icon' }, ['⚠']),
            el('span', {}, [
                'Your GPU doesn\'t support the WebGPU ',
                el('code', {}, ['shader-f16']),
                ' feature. Variants using f16 quantization are disabled — ',
                'pick an f32 variant instead.',
            ]),
        ]));
    }
    dialog.appendChild(filtersEl);
    dialog.appendChild(tableWrap);

    // `grid` field aliases tbody for back-compat with rerenderGrid logic;
    // `loadBtn`/`summaryEl` retained as null so legacy destructuring doesn't crash.
    return { dialog, grid: tbody, thead, summaryEl: null, loadBtn: null, filtersEl };
}

/** Rebuild the thead in place so the active-sort indicator (▲/▼) tracks
 *  `ctx.filters.sort`. Called from openPicker's onFiltersChange. */
export function rerenderTableHeader(thead, ctx) {
    if (!thead) return;
    const sortHeader = (label, asc, desc) => {
        const isActive = ctx.filters.sort === asc || ctx.filters.sort === desc;
        const indicator = ctx.filters.sort === asc ? ' ▲'
            : ctx.filters.sort === desc ? ' ▼'
            : '';
        return el('th', {
            class: `mp-th-sortable ${isActive ? 'is-active' : ''}`,
            role: 'button',
            tabindex: '0',
            onClick: () => {
                const next = ctx.filters.sort === asc ? desc : asc;
                ctx.onFiltersChange({ ...ctx.filters, sort: next });
            },
        }, [label + indicator]);
    };
    thead.replaceChildren(el('tr', {}, [
        sortHeader('Model', 'az', 'za'),
        sortHeader('Provider', 'provider-az', 'provider-za'),
        el('th', { class: 'mp-th-variant' }, ['Variant']),
        sortHeader('Size', 'smallest', 'largest'),
        el('th', { class: 'mp-th-caps' }, ['Capabilities']),
        el('th', { class: 'mp-th-status' }, ['Status']),
        el('th', { class: 'mp-th-actions' }, ['Actions']),
    ]));
}

export { renderTableRow as renderCard, smallestVramMb };

const FILTERS_LS_KEY = 'gizza:picker-filters';
const FAVORITES_LS_KEY = 'gizza:picker-favorites';

export function readPersistedFavorites({ localStorage: storage = (typeof localStorage !== 'undefined' ? localStorage : null) } = {}) {
    try {
        if (!storage) return new Set();
        const raw = storage.getItem(FAVORITES_LS_KEY);
        if (!raw) return new Set();
        const parsed = JSON.parse(raw);
        return new Set(Array.isArray(parsed) ? parsed : []);
    } catch (_e) {
        return new Set();
    }
}

export function writePersistedFavorites(favorites, { localStorage: storage = (typeof localStorage !== 'undefined' ? localStorage : null) } = {}) {
    try {
        if (!storage) return;
        storage.setItem(FAVORITES_LS_KEY, JSON.stringify([...favorites]));
    } catch (_e) {
        // ignore quota failures
    }
}

const DEFAULT_FILTERS = {
    search: '',
    sizes: [],
    families: [],
    visionOnly: false,
    favoritesOnly: false,
    sort: 'downloaded-popular',
};

export function readPersistedFilters({ localStorage: storage = (typeof localStorage !== 'undefined' ? localStorage : null) } = {}) {
    try {
        if (!storage) return { ...DEFAULT_FILTERS };
        const raw = storage.getItem(FILTERS_LS_KEY);
        if (!raw) return { ...DEFAULT_FILTERS };
        const parsed = JSON.parse(raw);
        return {
            ...DEFAULT_FILTERS,
            ...parsed,
            search: '', // never persist search
        };
    } catch (_e) {
        return { ...DEFAULT_FILTERS };
    }
}

export function writePersistedFilters(filters, { localStorage: storage = (typeof localStorage !== 'undefined' ? localStorage : null) } = {}) {
    try {
        if (!storage) return;
        const { search: _drop, ...persistable } = filters;
        storage.setItem(FILTERS_LS_KEY, JSON.stringify(persistable));
    } catch (_e) {
        // ignore quota failures
    }
}

function familyMatches(group, families) {
    if (!families.length) return true;
    if (group.family === 'Other') return families.includes('Other');
    // Match the chip name (Llama, Qwen, ...) against the group's base_id prefix.
    return families.some((f) => group.base_id.toLowerCase().startsWith(f.toLowerCase()));
}

function sizeMatches(group, sizes) {
    if (!sizes.length) return true;
    const mb = smallestVramMb(group);
    return sizes.some((id) => {
        const tier = SIZE_TIERS.find((t) => t.id === id);
        if (!tier) return false;
        if (tier.max_mb && mb > tier.max_mb) return false;
        if (tier.min_mb && mb < tier.min_mb) return false;
        return true;
    });
}

export function applyFilters(groups, filters, popularity, cached, favorites = new Set()) {
    const search = filters.search.trim().toLowerCase();
    let filtered = groups.filter((g) => {
        if (search && !g.base_id.toLowerCase().includes(search) && !g.family.toLowerCase().includes(search)) return false;
        if (filters.visionOnly && !g.has_vision) return false;
        if (filters.favoritesOnly && !favorites.has(g.base_id)) return false;
        if (!familyMatches(g, filters.families)) return false;
        if (!sizeMatches(g, filters.sizes)) return false;
        return true;
    });

    const popOf = (g) => g.variants.reduce((s, v) => s + (popularity[v.id]?.downloads || 0), 0);
    const sizeOf = (g) => smallestVramMb(g);

    switch (filters.sort) {
        case 'popular':
            filtered.sort((a, b) => popOf(b) - popOf(a));
            break;
        case 'smallest':
            filtered.sort((a, b) => sizeOf(a) - sizeOf(b));
            break;
        case 'largest':
            filtered.sort((a, b) => sizeOf(b) - sizeOf(a));
            break;
        case 'az':
            filtered.sort((a, b) => a.base_id.localeCompare(b.base_id));
            break;
        case 'za':
            filtered.sort((a, b) => b.base_id.localeCompare(a.base_id));
            break;
        case 'provider-az':
            filtered.sort((a, b) => (a.family || '').localeCompare(b.family || ''));
            break;
        case 'provider-za':
            filtered.sort((a, b) => (b.family || '').localeCompare(a.family || ''));
            break;
        case 'downloaded-popular':
        default:
            filtered.sort((a, b) => {
                const af = favorites.has(a.base_id) ? 0 : 1;
                const bf = favorites.has(b.base_id) ? 0 : 1;
                if (af !== bf) return af - bf;
                const ac = cached.has(a.base_id) ? 0 : 1;
                const bc = cached.has(b.base_id) ? 0 : 1;
                if (ac !== bc) return ac - bc;
                return popOf(b) - popOf(a);
            });
            break;
    }
    return filtered;
}

function variantSummaryText(group, variant, popularity) {
    const pop = popularity[variant.id]?.downloads
        ? formatDownloads(popularity[variant.id].downloads)
        : null;
    const parts = [
        `**${group.base_id}**`,
        variant.label,
        formatBytes(variant.vram_mb),
        pop,
    ].filter(Boolean);
    return parts.join(' · ');
}

function setSummary(summaryEl, group, variant, popularity) {
    summaryEl.innerHTML = '';
    summaryEl.classList.remove('placeholder');
    const text = variantSummaryText(group, variant, popularity);
    // Render with bold around the model name (preserve simple emphasis)
    const parts = text.split(' · ');
    const titlePart = parts[0].replace(/^\*\*|\*\*$/g, '');
    summaryEl.appendChild(el('strong', {}, [titlePart]));
    if (parts.length > 1) {
        summaryEl.appendChild(el('span', { class: 'mp-footer-meta' }, [' · ' + parts.slice(1).join(' · ')]));
    }
}

function setSummaryPlaceholder(summaryEl) {
    summaryEl.innerHTML = '';
    summaryEl.classList.add('placeholder');
    summaryEl.textContent = 'Pick a model to continue.';
}

function updateResultCount(filtersEl, total, visible, onClear) {
    const slot = filtersEl.querySelector('[data-result-count]');
    slot.innerHTML = '';
    slot.appendChild(document.createTextNode(`${visible} of ${total} models`));
    if (visible !== total) {
        slot.appendChild(document.createTextNode(' · '));
        const link = el('a', { href: '#', onClick: (e) => { e.preventDefault(); onClear(); } }, ['Clear filters']);
        slot.appendChild(link);
    }
}

/** Parse a percent value out of a WebLLM init-progress text like
 *  "Loading model from cache[12/24]: 50% complete, 8 secs elapsed.". Returns
 *  `null` when no integer percent is present. */
function parsePercentFromText(text) {
    if (typeof text !== 'string') return null;
    const m = text.match(/(\d+(?:\.\d+)?)\s*%/);
    if (m) return Math.max(0, Math.min(100, parseFloat(m[1])));
    const frac = text.match(/\[(\d+)\/(\d+)\]/);
    if (frac) {
        const a = parseInt(frac[1], 10);
        const b = parseInt(frac[2], 10);
        if (b > 0) return Math.max(0, Math.min(100, (a / b) * 100));
    }
    return null;
}

/**
 * Open the model picker. The picker owns the full load/unload lifecycle while
 * open: clicking Load downloads + loads in-modal with per-row progress;
 * clicking Unload on the active row releases the engine; closing (✕/Esc) does
 * NOT cancel a load in flight — it keeps running in the shared engine module.
 *
 * Caller provides:
 *   loadEngine(modelId, onProgress) → Promise<void>
 *   unloadEngine()                  → Promise<void>
 *   getCurrentModelId()             → string|null  (read latest active model)
 *   onActiveChange(modelId|null)    — invoked every time the active model
 *                                      changes (on successful load or unload)
 *
 * Resolves with `{ model_id }` (the active model when the picker closes) or
 * `null` if nothing is loaded at close time.
 */
export async function openPicker({
    prebuiltList,
    getCurrentModelId = () => null,
    loadEngine,
    unloadEngine,
    onActiveChange = () => {},
} = {}) {
    const groups = groupModels(prebuiltList);
    const popularity = await fetchHfPopularity();
    const currentModelId = getCurrentModelId();
    const { cached } = await getCachedAndActive(groups, currentModelId);
    let active = currentModelId;

    // Resolve each brain tier to a concrete model_id based on whether the
    // user's WebGPU adapter exposes `shader-f16`. This is a one-shot probe;
    // the result feeds renderTierCards via ctx.tiers.
    const f16Supported = await probeShaderF16();
    const tiers = BRAIN_TIERS.map((t) => resolveTierVariant(t, f16Supported));

    let filters = readPersistedFilters();
    let selection = null; // { base_id, variant }
    const favorites = readPersistedFavorites();

    // In-modal load state. Only one load may be in flight at a time.
    let loading = null;       // { variantId, baseId, text, percent }
    let errorForBase = null;  // last failure: which base_id
    let errorText = null;     // last failure: message to surface

    // Picker view mode. Default to the curated tier-card landing; the user can
    // flip to the full table via the "Custom brain →" link.
    let view = 'tiers';

    return new Promise((resolve) => {
        const ctx = {
            groups, popularity, cached, active, selection, filters, favorites,
            loading, errorForBase, errorText, view, tiers, f16Supported,
            onClose: () => close(),
            onFiltersChange: (next) => {
                filters = next;
                ctx.filters = filters;
                writePersistedFilters(filters);
                rerenderHeader();
                rerenderGrid();
            },
            onFavoriteToggle: (baseId) => {
                if (favorites.has(baseId)) favorites.delete(baseId);
                else favorites.add(baseId);
                writePersistedFavorites(favorites);
                rerenderEverything();
            },
            onDeleteCached: async (group) => {
                if (!window.confirm(`Delete cached ${group.base_id}?`)) return;
                await deleteCachedModel(group);
                const refresh = await getCachedAndActive(groups, active);
                cached.clear();
                for (const id of refresh.cached) cached.add(id);
                rerenderEverything();
            },
            onSwitchView: (next) => {
                view = next === 'custom' ? 'custom' : 'tiers';
                ctx.view = view;
                rebuildDialogBody();
            },
            onLoadModelId: (modelId) => {
                const found = resolveTierGroupVariant(groups, modelId);
                if (found) performLoad(found.group, found.variant);
            },
            onUnload: () => performUnload(),
        };

        const dom = renderPickerDom(ctx);
        document.body.appendChild(dom.dialog);

        // Switch views by rebuilding the dialog's children in place. The
        // dialog element itself is preserved so the `close` listener stays
        // attached and `showModal` doesn't need to fire again. In tier mode
        // the body is fully rendered by renderPickerDom; in custom mode the
        // table skeleton lands but rows are populated by rerenderGrid().
        function rebuildDialogBody() {
            syncCtx();
            const fresh = renderPickerDom(ctx);
            dom.dialog.replaceChildren(...fresh.dialog.children);
            dom.grid = fresh.grid;
            dom.thead = fresh.thead;
            dom.summaryEl = fresh.summaryEl;
            dom.loadBtn = fresh.loadBtn;
            dom.filtersEl = fresh.filtersEl;
            dom.view = fresh.view;
            if (view === 'custom') rerenderGrid();
        }

        function syncCtx() {
            ctx.active = active;
            ctx.loading = loading;
            ctx.errorForBase = errorForBase;
            ctx.errorText = errorText;
        }

        async function performLoad(group, variant) {
            if (loading) return; // another load already in flight
            if (typeof loadEngine !== 'function') return;
            errorForBase = null;
            errorText = null;
            loading = { variantId: variant.id, baseId: group.base_id, text: 'Starting…', percent: null };
            syncCtx();
            rerenderGrid();

            const onProgress = (text) => {
                // The picker may have closed mid-load; skip DOM updates if so.
                if (!dom.dialog.isConnected) return;
                if (!loading || loading.variantId !== variant.id) return;
                loading.text = text || 'Downloading…';
                loading.percent = parsePercentFromText(text);
                // Lightweight in-place update: avoid full grid rerender on
                // every progress tick (which would discard select/scroll).
                updateRowProgressInPlace(group.base_id, loading);
            };

            let loadOk = false;
            try {
                await loadEngine(variant.id, onProgress);
                active = variant.id;
                cached.add(group.base_id);
                onActiveChange(active);
                loadOk = true;
            } catch (e) {
                errorForBase = group.base_id;
                errorText = String(e?.message ?? e);
            } finally {
                loading = null;
                if (dom.dialog.isConnected) {
                    syncCtx();
                    rerenderGrid();
                }
            }
            // Auto-close the picker on first successful load so the user
            // lands back on the chat surface ready to type. Errors keep the
            // picker open so the user can read the failure + retry.
            if (loadOk && dom.dialog.isConnected) close();
        }

        async function performUnload() {
            if (loading) return;
            if (typeof unloadEngine !== 'function') return;
            try {
                await unloadEngine();
            } catch (_e) { /* unload is best-effort */ }
            active = null;
            onActiveChange(null);
            syncCtx();
            if (dom.dialog.isConnected) rerenderGrid();
        }

        function updateRowProgressInPlace(baseId, loadingState) {
            // Update the per-row bar (custom-table view).
            const row = dom.grid?.querySelector(
                `tr.mp-row[data-base-id="${CSS.escape(baseId)}"]`,
            );
            if (row) {
                const text = row.querySelector('.mp-load-progress-text');
                const bar = row.querySelector('.mp-load-progress-bar');
                if (text) text.textContent = loadingState.text || 'Downloading…';
                if (bar) {
                    const pct = loadingState.percent;
                    bar.style.width = typeof pct === 'number'
                        ? `${Math.max(0, Math.min(100, pct))}%`
                        : '0%';
                }
            }
            // Update the matching tier card (tier-card view). The tier card
            // matches on the exact model_id, not the base_id, so we look it
            // up by data-model-id.
            const card = dom.dialog.querySelector(
                `.mp-tier-card[data-model-id="${CSS.escape(loadingState.variantId)}"]`,
            );
            if (card) {
                const text = card.querySelector('.mp-tier-progress-text');
                const bar = card.querySelector('.mp-tier-progress-bar');
                if (text) text.textContent = loadingState.text || 'Downloading…';
                if (bar) {
                    const pct = loadingState.percent;
                    bar.style.width = typeof pct === 'number'
                        ? `${Math.max(0, Math.min(100, pct))}%`
                        : '0%';
                }
            }
        }

        function rerenderGrid() {
            // Tier-card view: rebuild only the .mp-tiers panel so the dialog
            // children aren't disturbed.
            if (view === 'tiers') {
                const old = dom.dialog.querySelector('.mp-tiers');
                if (old) old.replaceWith(renderTierCards(ctx));
                return;
            }
            const filtered = applyFilters(groups, filters, popularity, cached, favorites);
            if (!dom.grid) return;
            dom.grid.innerHTML = '';
            if (filtered.length === 0) {
                const emptyRow = el('tr', {}, [el('td', { colspan: '7', class: 'mp-empty' }, [
                    'No models match these filters · ',
                    el('a', { onClick: clearFilters }, ['Clear filters']),
                ])]);
                dom.grid.appendChild(emptyRow);
            } else {
                for (const g of filtered) {
                    const row = renderTableRow(g, {
                        cached, active, popularity, selection, favorites,
                        loading, errorForBase, errorText, f16Supported,
                        onFavoriteToggle: ctx.onFavoriteToggle,
                        onDeleteCached: ctx.onDeleteCached,
                    });
                    bindRowEvents(row, g);
                    dom.grid.appendChild(row);
                }
            }
            if (dom.filtersEl) {
                updateResultCount(dom.filtersEl, groups.length, filtered.length, clearFilters);
            }
        }

        function clearFilters() {
            filters = { ...DEFAULT_FILTERS };
            ctx.filters = filters;
            writePersistedFilters(filters);
            const newDom = renderPickerDom({ ...ctx, filters });
            dom.dialog.replaceChildren(...newDom.dialog.children);
            dom.grid = newDom.grid;
            dom.thead = newDom.thead;
            dom.summaryEl = newDom.summaryEl;
            dom.loadBtn = newDom.loadBtn;
            dom.filtersEl = newDom.filtersEl;
            selection = null;
            ctx.selection = null;
            rerenderGrid();
        }

        function rerenderHeader() {
            rerenderTableHeader(dom.thead, ctx);
        }

        function bindRowEvents(row, group) {
            const select = row.querySelector('.mp-quality-select');
            let currentVariant = group.variants.find((v) => v.id === select?.value)
                || pickInitialVariant(group, f16Supported);

            const loadBtn = row.querySelector('.mp-load-btn');

            const refreshLoadGate = () => {
                if (!loadBtn) return;
                const blocked = !f16Supported && requiresShaderF16(currentVariant?.quant);
                loadBtn.disabled = !!(loading || blocked);
                if (blocked) {
                    loadBtn.title = 'shader-f16 not supported by your GPU — pick a different variant';
                } else if (loading) {
                    loadBtn.title = 'Another model is loading…';
                } else {
                    loadBtn.title = '';
                }
            };

            select?.addEventListener('change', () => {
                currentVariant = group.variants.find((v) => v.id === select.value) || currentVariant;
                const sizeEl = row.querySelector('.mp-size-value');
                if (sizeEl) sizeEl.textContent = formatBytes(currentVariant.vram_mb);
                refreshLoadGate();
            });

            loadBtn?.addEventListener('click', () => {
                if (loading) return;
                if (!f16Supported && requiresShaderF16(currentVariant?.quant)) return;
                performLoad(group, currentVariant);
            });

            const unloadBtn = row.querySelector('.mp-unload-btn');
            unloadBtn?.addEventListener('click', () => {
                if (loading) return;
                performUnload();
            });
        }

        function close() {
            try { dom.dialog.close(); } catch (_e) {}
            dom.dialog.remove();
            resolve(active ? { model_id: active } : null);
        }

        // Esc closes via <dialog> default; ensure we clean up DOM and resolve.
        dom.dialog.addEventListener('close', () => {
            if (dom.dialog.parentElement) {
                dom.dialog.remove();
                resolve(active ? { model_id: active } : null);
            }
        });

        rerenderGrid();
        dom.dialog.showModal();
    });
}
