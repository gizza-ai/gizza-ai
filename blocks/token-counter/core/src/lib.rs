//! token-counter core — pure compute, shared by the chat skill block and the web page.
//! Counts LLM tokens for pasted text with real BPE tokenization (`tiktoken-rs`),
//! then estimates the prompt (input) cost and context-window usage for a chosen
//! model from an embedded pricing snapshot. No wafer/wasm-bindgen deps.

/// One model's tokenizer + pricing metadata. Prices are $ per 1,000,000 tokens.
struct ModelInfo {
    id: &'static str,
    label: &'static str,
    provider: &'static str,
    /// tiktoken encoding used to count: "o200k_base" or "cl100k_base".
    encoding: &'static str,
    /// true when the encoding is the model's real tokenizer (OpenAI); false when
    /// it's an approximation of a proprietary tokenizer (Anthropic / Google).
    exact: bool,
    input_per_m: f64,
    output_per_m: f64,
    context: usize,
}

/// Embedded pricing snapshot (2026-07-13, $ per 1M tokens, input / output).
/// OpenAI counts are exact (real tiktoken encoding); Anthropic/Google counts are
/// an o200k_base approximation of their proprietary tokenizers. Keep in sync with
/// docs/checks/2026-07-13-improve-token-counter-competitor-analysis.md.
const MODELS: &[ModelInfo] = &[
    // OpenAI — exact (o200k_base)
    ModelInfo { id: "gpt-5.5",       label: "GPT-5.5",          provider: "OpenAI",    encoding: "o200k_base", exact: true,  input_per_m: 5.0,  output_per_m: 30.0, context: 400_000 },
    ModelInfo { id: "gpt-5",         label: "GPT-5",            provider: "OpenAI",    encoding: "o200k_base", exact: true,  input_per_m: 1.25, output_per_m: 10.0, context: 400_000 },
    ModelInfo { id: "gpt-4.1",       label: "GPT-4.1",          provider: "OpenAI",    encoding: "o200k_base", exact: true,  input_per_m: 2.0,  output_per_m: 8.0,  context: 1_000_000 },
    ModelInfo { id: "gpt-4.1-mini",  label: "GPT-4.1 mini",     provider: "OpenAI",    encoding: "o200k_base", exact: true,  input_per_m: 0.40, output_per_m: 1.60, context: 1_000_000 },
    ModelInfo { id: "gpt-4o",        label: "GPT-4o",           provider: "OpenAI",    encoding: "o200k_base", exact: true,  input_per_m: 2.50, output_per_m: 10.0, context: 128_000 },
    ModelInfo { id: "gpt-4o-mini",   label: "GPT-4o mini",      provider: "OpenAI",    encoding: "o200k_base", exact: true,  input_per_m: 0.15, output_per_m: 0.60, context: 128_000 },
    // OpenAI — exact (cl100k_base)
    ModelInfo { id: "gpt-4-turbo",   label: "GPT-4 Turbo",      provider: "OpenAI",    encoding: "cl100k_base", exact: true, input_per_m: 10.0, output_per_m: 30.0, context: 128_000 },
    ModelInfo { id: "gpt-3.5-turbo", label: "GPT-3.5 Turbo",    provider: "OpenAI",    encoding: "cl100k_base", exact: true, input_per_m: 0.50, output_per_m: 1.50, context: 16_385 },
    // Anthropic — approx (o200k_base)
    ModelInfo { id: "claude-opus-4.8",   label: "Claude Opus 4.8",  provider: "Anthropic", encoding: "o200k_base", exact: false, input_per_m: 5.0, output_per_m: 25.0, context: 200_000 },
    ModelInfo { id: "claude-sonnet-5",   label: "Claude Sonnet 5",  provider: "Anthropic", encoding: "o200k_base", exact: false, input_per_m: 3.0, output_per_m: 15.0, context: 200_000 },
    ModelInfo { id: "claude-haiku-4.5",  label: "Claude Haiku 4.5", provider: "Anthropic", encoding: "o200k_base", exact: false, input_per_m: 1.0, output_per_m: 5.0,  context: 200_000 },
    // Google — approx (o200k_base)
    ModelInfo { id: "gemini-3-pro",      label: "Gemini 3 Pro",     provider: "Google",    encoding: "o200k_base", exact: false, input_per_m: 2.0,  output_per_m: 12.0, context: 1_000_000 },
    ModelInfo { id: "gemini-2.5-flash",  label: "Gemini 2.5 Flash", provider: "Google",    encoding: "o200k_base", exact: false, input_per_m: 0.30, output_per_m: 2.50, context: 1_000_000 },
];

/// The pricing-snapshot date embedded above (shown in output).
pub const PRICING_DATE: &str = "2026-07-13";

/// The enum variant list, in declaration order — used by the descriptor so the
/// chat schema, CLI, and page dropdown can't drift from the pricing table.
pub fn model_ids() -> Vec<&'static str> {
    MODELS.iter().map(|m| m.id).collect()
}

/// The default model id (first OpenAI general-purpose model most users reach for).
pub const DEFAULT_MODEL: &str = "gpt-4o";

fn model_by_id(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

/// Format an integer with thousands separators (1234567 -> "1,234,567").
fn commas(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

/// Format a per-1M price like "$2.50", "$0.15", "$10.00".
fn fmt_price(p: f64) -> String {
    format!("${:.2}", p)
}

/// Count tokens for `text` with the tokenizer for `model`, then report the count,
/// character count, estimated input cost, output price, and context-window usage.
/// `model` is a model id from [`model_ids`]; empty falls back to [`DEFAULT_MODEL`].
pub fn count(text: &str, model: &str) -> Result<String, String> {
    let model = if model.trim().is_empty() { DEFAULT_MODEL } else { model.trim() };
    let info = model_by_id(model).ok_or_else(|| {
        format!(
            "unknown model '{}'. Supported: {}.",
            model,
            model_ids().join(", ")
        )
    })?;

    let bpe = match info.encoding {
        "cl100k_base" => tiktoken_rs::cl100k_base(),
        _ => tiktoken_rs::o200k_base(),
    }
    .map_err(|e| format!("failed to load {} tokenizer: {e}", info.encoding))?;

    let tokens = bpe.encode_ordinary(text).len();
    let chars = text.chars().count();

    let exactness = if info.exact { "exact" } else { "approx" };
    let input_cost = tokens as f64 * info.input_per_m / 1_000_000.0;

    let pct = if info.context > 0 {
        tokens as f64 / info.context as f64 * 100.0
    } else {
        0.0
    };
    let pct_str = if pct == 0.0 {
        "0% used".to_string()
    } else if pct < 0.01 {
        "<0.01% used".to_string()
    } else {
        format!("{:.2}% used", pct)
    };

    let tokenizer_line = if info.exact {
        format!("Tokenizer: {} — exact count", info.encoding)
    } else {
        format!(
            "Tokenizer: {} — approximate ({}'s tokenizer is proprietary)",
            info.encoding, info.label
        )
    };

    let mut out = String::new();
    out.push_str(&format!("Model: {} ({})\n", info.label, info.provider));
    out.push_str(&format!("{}\n", tokenizer_line));
    out.push_str(&format!("Tokens: {} ({})\n", commas(tokens), exactness));
    out.push_str(&format!("Characters: {}\n", commas(chars)));
    out.push('\n');
    out.push_str("Estimated cost\n");
    out.push_str(&format!(
        "  Input:  ${:.6}  (at {} / 1M tokens)\n",
        input_cost,
        fmt_price(info.input_per_m)
    ));
    out.push_str(&format!(
        "  Output: {} / 1M tokens (reference)\n",
        fmt_price(info.output_per_m)
    ));
    out.push('\n');
    out.push_str(&format!(
        "Context window: {} tokens ({})\n",
        commas(info.context),
        pct_str
    ));
    out.push('\n');
    out.push_str(&format!(
        "Note: pricing is a {} estimate — verify on the provider's pricing page.",
        PRICING_DATE
    ));

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_tokens_exact_openai() {
        let out = count("hello world", "gpt-4o").unwrap();
        assert!(out.contains("Model: GPT-4o (OpenAI)"), "{out}");
        assert!(out.contains("Tokenizer: o200k_base — exact count"), "{out}");
        assert!(out.contains("(exact)"), "{out}");
        // "hello world" is 2 tokens under o200k_base.
        assert!(out.contains("Tokens: 2 (exact)"), "{out}");
        assert!(out.contains("Characters: 11"), "{out}");
        assert!(out.contains("(at $2.50 / 1M tokens)"), "{out}");
        assert!(out.contains("Context window: 128,000 tokens"), "{out}");
    }

    #[test]
    fn approx_model_labels_approx() {
        let out = count("hello world", "claude-opus-4.8").unwrap();
        assert!(out.contains("Model: Claude Opus 4.8 (Anthropic)"), "{out}");
        assert!(out.contains("approximate"), "{out}");
        assert!(out.contains("(approx)"), "{out}");
    }

    #[test]
    fn cl100k_model_uses_cl100k() {
        let out = count("hello", "gpt-3.5-turbo").unwrap();
        assert!(out.contains("Tokenizer: cl100k_base — exact count"), "{out}");
    }

    #[test]
    fn empty_model_defaults() {
        let out = count("hi", "").unwrap();
        assert!(out.contains("Model: GPT-4o (OpenAI)"), "{out}");
    }

    #[test]
    fn unknown_model_errors() {
        let err = count("hi", "gpt-9000").unwrap_err();
        assert!(err.contains("unknown model"), "{err}");
    }

    #[test]
    fn commas_formats() {
        assert_eq!(commas(0), "0");
        assert_eq!(commas(999), "999");
        assert_eq!(commas(1000), "1,000");
        assert_eq!(commas(1234567), "1,234,567");
    }
}
