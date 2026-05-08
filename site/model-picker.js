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
