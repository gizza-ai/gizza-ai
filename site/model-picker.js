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
