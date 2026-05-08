//! gizza-ai model picker — groups, fetches, renders. See spec at
//! docs/superpowers/specs/2026-05-08-gizza-ai-model-picker-redesign-design.md.

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
    // <base>-<quant>-MLC[-<ctx>] → <base>
    return modelId.replace(/-q\d+f\d+(_\d+)?-MLC(-\d+k(_cs\d+k)?)?$/, '');
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

const SIZE_TIERS = [
    { id: 'small', label: 'Small (<2 GB)', max_mb: 2048 },
    { id: 'medium', label: 'Medium (2–5 GB)', min_mb: 2048, max_mb: 5120 },
    { id: 'large', label: 'Large (5+ GB)', min_mb: 5120 },
];

const FAMILY_CHIP_OPTIONS = ['Llama', 'Qwen', 'Phi', 'Hermes', 'Gemma', 'Mistral', 'Other'];

const SORT_OPTIONS = [
    { value: 'downloaded-popular', label: 'Already downloaded, then most popular' },
    { value: 'popular', label: 'Most popular' },
    { value: 'smallest', label: 'Smallest first' },
    { value: 'largest', label: 'Largest first' },
    { value: 'az', label: 'A–Z' },
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

function renderCard(group, ctx) {
    const initialVariant = ctx.selection?.base_id === group.base_id
        ? ctx.selection.variant
        : group.variants[Math.floor(group.variants.length / 2)] || group.variants[0];
    const isCached = ctx.cached.has(group.base_id);
    const isActive = group.variants.some((v) => v.id === ctx.active);
    const popularity = group.variants
        .map((v) => ctx.popularity[v.id]?.downloads || 0)
        .reduce((a, b) => a + b, 0);

    const card = el('div', {
        class: ['mp-card', isActive ? 'active' : '', ctx.selection?.base_id === group.base_id ? 'selected' : ''].filter(Boolean).join(' '),
        'data-base-id': group.base_id,
    }, [
        el('div', { class: 'mp-card-top' }, [
            el('div', {}, [
                el('div', { class: 'mp-card-title' }, [
                    group.base_id,
                    group.has_tools ? el('span', { class: 'mp-badge tools' }, ['🔧 tools']) : null,
                    isCached && !isActive ? el('span', { class: 'mp-badge cached' }, ['✓ Downloaded']) : null,
                    isActive ? el('span', { class: 'mp-badge active' }, ['✓ Active']) : null,
                ]),
                el('div', { class: 'mp-card-subtitle' }, [
                    [group.family, group.params_label].filter(Boolean).join(' · '),
                ]),
            ]),
            group.hf_url ? el('a', {
                class: 'mp-card-link',
                href: group.hf_url,
                target: '_blank',
                rel: 'noopener',
                title: 'Open on HuggingFace',
                onClick: (e) => e.stopPropagation(),
            }, ['HF ↗']) : null,
        ]),
        el('div', { class: 'mp-card-stats' }, [
            el('span', {}, [el('strong', {}, [formatBytes(initialVariant?.vram_mb)])]),
            popularity ? el('span', {}, [formatDownloads(popularity)]) : null,
            group.ctx ? el('span', {}, [`${Math.round(group.ctx / 1024)}k ctx`]) : null,
        ]),
        !isActive && group.variants.length > 1 ? renderQualityPicker(group, initialVariant) : null,
    ]);
    return card;
}

function renderQualityPicker(group, initialVariant) {
    const wrap = el('div', { class: 'mp-quality' });
    for (const v of group.variants) {
        const btn = el('button', {
            type: 'button',
            class: v.id === initialVariant?.id ? 'selected' : '',
            'data-variant-id': v.id,
            onClick: (e) => e.stopPropagation(),
        }, [
            v.label,
            el('span', { class: 'mp-quality-sub' }, [v.sublabel]),
        ]);
        wrap.appendChild(btn);
    }
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
        el('div', { class: 'mp-chip-group', 'aria-label': 'Size filter' }, SIZE_TIERS.map((tier) =>
            el('button', {
                type: 'button',
                class: `mp-chip ${ctx.filters.sizes.includes(tier.id) ? 'active' : ''}`,
                onClick: () => {
                    const next = ctx.filters.sizes.includes(tier.id)
                        ? ctx.filters.sizes.filter((x) => x !== tier.id)
                        : [...ctx.filters.sizes, tier.id];
                    ctx.onFiltersChange({ ...ctx.filters, sizes: next });
                },
            }, [tier.label]))),
        el('div', { class: 'mp-chip-group', 'aria-label': 'Family filter' }, FAMILY_CHIP_OPTIONS.map((fam) =>
            el('button', {
                type: 'button',
                class: `mp-chip ${ctx.filters.families.includes(fam) ? 'active' : ''}`,
                onClick: () => {
                    const next = ctx.filters.families.includes(fam)
                        ? ctx.filters.families.filter((x) => x !== fam)
                        : [...ctx.filters.families, fam];
                    ctx.onFiltersChange({ ...ctx.filters, families: next });
                },
            }, [fam]))),
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

    // Grid
    const grid = el('div', { class: 'mp-grid' });
    const gridWrap = el('div', { class: 'mp-grid-wrap' }, [grid]);

    // Footer
    const summaryEl = el('div', { class: 'mp-footer-summary placeholder' }, ['Pick a model to continue.']);
    const loadBtn = el('button', { class: 'primary', type: 'button', disabled: true, onClick: ctx.onLoad }, ['Load model']);
    const footer = el('div', { class: 'mp-footer' }, [summaryEl, loadBtn]);

    dialog.appendChild(header);
    dialog.appendChild(filtersEl);
    dialog.appendChild(gridWrap);
    dialog.appendChild(footer);

    return { dialog, grid, summaryEl, loadBtn, filtersEl };
}

export { renderCard, smallestVramMb };
