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

const TOOL_SUPPORT_HINTS = ['Hermes-2', 'Hermes-3', 'Qwen2.5', 'Llama-3-Groq', 'functionary'];

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
                has_tools: false,
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
            if (TOOL_SUPPORT_HINTS.some((h) => entry.model_id.includes(h))) group.has_tools = true;
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
        if (TOOL_SUPPORT_HINTS.some((h) => entry.model_id.includes(h))) group.has_tools = true;
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

function renderTableRow(group, ctx) {
    const initialVariant = ctx.selection?.base_id === group.base_id
        ? ctx.selection.variant
        : group.variants[Math.floor(group.variants.length / 2)] || group.variants[0];
    const isCached = ctx.cached.has(group.base_id);
    const isActive = group.variants.some((v) => v.id === ctx.active);

    const tr = el('tr', {
        class: ['mp-row', isActive ? 'is-active' : '', isCached && !isActive ? 'is-cached' : ''].filter(Boolean).join(' '),
        'data-base-id': group.base_id,
    });

    // Model cell — name + HF link + favorite star + delete-cached trash
    // (the last two preserved from the pre-rebase card layout, PR #42).
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
    const trashBtn = isCached && !isActive ? el('button', {
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
            group.base_id,
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

    // Variant dropdown.
    const variantCell = el('td', { class: 'mp-cell-variant' });
    variantCell.appendChild(renderQualityPicker(group, initialVariant));
    tr.appendChild(variantCell);

    // Size — updates when variant changes.
    tr.appendChild(el('td', { class: 'mp-cell-size' }, [
        el('strong', { class: 'mp-size-value' }, [formatBytes(initialVariant?.vram_mb)]),
    ]));

    // Capabilities — Tools, Vision (vision detection lives in group.has_vision if set).
    const caps = el('td', { class: 'mp-cell-caps' });
    if (group.has_tools) caps.appendChild(el('span', { class: 'mp-badge tools' }, ['🔧 tools']));
    if (group.ctx) caps.appendChild(el('span', { class: 'mp-badge ctx' }, [`${Math.round(group.ctx / 1024)}k ctx`]));
    tr.appendChild(caps);

    // Status.
    let statusText, statusClass;
    if (isActive) { statusText = '✓ Loaded'; statusClass = 'is-loaded'; }
    else if (isCached) { statusText = '✓ Cached'; statusClass = 'is-cached'; }
    else { statusText = '—'; statusClass = 'is-empty'; }
    tr.appendChild(el('td', { class: `mp-cell-status ${statusClass}` }, [statusText]));

    // Actions — Download + Load buttons.
    const actionCell = el('td', { class: 'mp-cell-actions' });
    const downloadBtn = el('button', {
        class: 'mp-action-btn mp-download-btn',
        type: 'button',
        disabled: isCached || isActive ? '' : undefined,
        title: isCached ? 'Already cached' : 'Download weights to browser cache',
    }, [isCached || isActive ? 'Cached' : 'Download']);
    const loadBtn = el('button', {
        class: 'mp-action-btn mp-load-btn primary',
        type: 'button',
        disabled: isActive ? '' : undefined,
        title: isActive ? 'Currently active' : 'Make this the active chat model',
    }, [isActive ? 'Loaded' : 'Load']);
    actionCell.appendChild(downloadBtn);
    actionCell.appendChild(loadBtn);
    tr.appendChild(actionCell);

    return tr;
}

function renderQualityPicker(group, initialVariant) {
    // Single dropdown replaces the dense variant-pill row. Each option
    // includes the quality label + its quantization sublabel so users can
    // still tell variants apart without four side-by-side buttons.
    const wrap = el('div', { class: 'mp-quality' });
    const select = el('select', {
        class: 'mp-quality-select',
        'aria-label': 'Variant',
        onClick: (e) => e.stopPropagation(),
    });
    for (const v of group.variants) {
        const opt = el('option', {
            value: v.id,
            'data-variant-id': v.id,
            selected: v.id === initialVariant?.id ? '' : undefined,
        }, [`${v.label} (${v.sublabel})`]);
        select.appendChild(opt);
    }
    wrap.appendChild(select);
    return wrap;
}

export function renderPickerDom(ctx) {
    // ctx: { groups, popularity, cached, active, selection, filters, onClose, onSelect, onLoad, onFiltersChange }
    const dialog = el('dialog', { id: 'model-picker' });

    // Header
    const header = el('div', { class: 'mp-header' }, [
        el('h2', {}, ['Choose a model']),
        el('button', { class: 'mp-close', 'aria-label': 'Close', onClick: ctx.onClose }, ['✕']),
    ]);

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
                checked: ctx.filters.toolsOnly ? true : false,
                onChange: (e) => ctx.onFiltersChange({ ...ctx.filters, toolsOnly: e.target.checked }),
            }),
            'Tools-capable',
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
    toolsOnly: false,
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
        if (filters.toolsOnly && !g.has_tools) return false;
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

/**
 * Open the model picker. Returns a Promise that resolves with
 *   { model_id }     when the user clicks Load
 *   null             when the user closes without choosing
 *
 * Caller is responsible for kicking off the actual model download with the
 * returned model_id — this picker only commits a selection.
 */
export async function openPicker({
    prebuiltList,
    currentModelId = null,
} = {}) {
    const groups = groupModels(prebuiltList);
    const popularity = await fetchHfPopularity();
    const { cached, active } = await getCachedAndActive(groups, currentModelId);

    let filters = readPersistedFilters();
    let selection = null; // { base_id, variant }
    const favorites = readPersistedFavorites();

    return new Promise((resolve) => {
        const ctx = {
            groups, popularity, cached, active, selection, filters, favorites,
            onClose: () => close(null),
            onSelect: () => {},
            onLoad: () => {
                if (!selection) return;
                close({ model_id: selection.variant.id });
            },
            onFiltersChange: (next) => {
                filters = next;
                // Keep ctx.filters live so per-render closures (e.g. the
                // sortable header click handler) read fresh state on each
                // click instead of the value captured at first render.
                ctx.filters = filters;
                writePersistedFilters(filters);
                rerenderHeader();
                rerenderGrid();
            },
            onFavoriteToggle: (baseId) => {
                if (favorites.has(baseId)) favorites.delete(baseId);
                else favorites.add(baseId);
                writePersistedFavorites(favorites);
                rerenderGrid();
            },
            onDeleteCached: async (group) => {
                if (!window.confirm(`Delete cached ${group.base_id}?`)) return;
                await deleteCachedModel(group);
                const refresh = await getCachedAndActive(groups, currentModelId);
                cached.clear();
                for (const id of refresh.cached) cached.add(id);
                rerenderGrid();
            },
        };

        const dom = renderPickerDom(ctx);
        document.body.appendChild(dom.dialog);

        function rerenderGrid() {
            const filtered = applyFilters(groups, filters, popularity, cached, favorites);
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
                        onFavoriteToggle: ctx.onFavoriteToggle,
                        onDeleteCached: ctx.onDeleteCached,
                    });
                    bindCardEvents(row, g);
                    dom.grid.appendChild(row);
                }
            }
            updateResultCount(dom.filtersEl, groups.length, filtered.length, clearFilters);
        }

        function clearFilters() {
            filters = { ...DEFAULT_FILTERS };
            ctx.filters = filters;
            writePersistedFilters(filters);
            // Reset filter UI by re-rendering the dialog from scratch is heavy;
            // simpler: replace the dialog contents.
            const newDom = renderPickerDom({ ...ctx, filters });
            dom.dialog.replaceChildren(...newDom.dialog.children);
            // Rebind references to the new DOM
            dom.grid = newDom.grid;
            dom.thead = newDom.thead;
            dom.summaryEl = newDom.summaryEl;
            dom.loadBtn = newDom.loadBtn;
            dom.filtersEl = newDom.filtersEl;
            // Reset selection
            selection = null;
            ctx.selection = null;
            rerenderGrid();
        }

        function rerenderHeader() {
            rerenderTableHeader(dom.thead, ctx);
        }

        function bindCardEvents(row, group) {
            const select = row.querySelector('.mp-quality-select');
            let currentVariant = group.variants.find((v) => v.id === select?.value)
                || group.variants[Math.floor(group.variants.length / 2)]
                || group.variants[0];

            select?.addEventListener('change', () => {
                currentVariant = group.variants.find((v) => v.id === select.value) || currentVariant;
                const sizeEl = row.querySelector('.mp-size-value');
                if (sizeEl) sizeEl.textContent = formatBytes(currentVariant.vram_mb);
            });

            const downloadBtn = row.querySelector('.mp-download-btn');
            downloadBtn?.addEventListener('click', () => {
                selection = { base_id: group.base_id, variant: currentVariant };
                ctx.selection = selection;
                close({ model_id: currentVariant.id, mode: 'download' });
            });

            const loadBtn = row.querySelector('.mp-load-btn');
            loadBtn?.addEventListener('click', () => {
                selection = { base_id: group.base_id, variant: currentVariant };
                ctx.selection = selection;
                close({ model_id: currentVariant.id, mode: 'load' });
            });
        }

        function close(value) {
            try { dom.dialog.close(); } catch (_e) {}
            dom.dialog.remove();
            resolve(value);
        }

        // Esc key closes via <dialog> default; we still need to clean up DOM
        dom.dialog.addEventListener('close', () => {
            if (dom.dialog.parentElement) {
                dom.dialog.remove();
                resolve(null);
            }
        });

        rerenderGrid();
        dom.dialog.showModal();
    });
}
