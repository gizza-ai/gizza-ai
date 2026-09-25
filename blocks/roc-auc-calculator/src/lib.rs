//! gizza-ai/roc-auc-calculator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Scores a binary
//! classifier from continuous scores plus class labels: Mann-Whitney AUC with
//! tie midranks, the DeLong interval, a threshold sweep and an optimal cutoff.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    labels: String,
    #[serde(default = "default_auto")]
    input_format: String,
    #[serde(default = "default_auto")]
    column_order: String,
    #[serde(default = "default_auto")]
    separator: String,
    #[serde(default = "default_auto")]
    header: String,
    #[serde(default)]
    positive_label: String,
    #[serde(default = "default_optimize")]
    optimize: String,
    #[serde(default = "default_cost_ratio")]
    cost_ratio: f64,
    #[serde(default)]
    threshold: String,
    #[serde(default = "default_confidence")]
    confidence_level: String,
    #[serde(default = "default_table_rows")]
    table_rows: f64,
    #[serde(default = "default_true")]
    plot: bool,
    #[serde(default = "default_decimals")]
    decimals: f64,
    #[serde(default)]
    percent: bool,
    #[serde(default = "default_format")]
    format: String,
}
fn default_auto() -> String {
    "auto".into()
}
fn default_optimize() -> String {
    "youden".into()
}
fn default_cost_ratio() -> f64 {
    1.0
}
fn default_confidence() -> String {
    "95".into()
}
fn default_table_rows() -> f64 {
    12.0
}
fn default_true() -> bool {
    true
}
fn default_decimals() -> f64 {
    4.0
}
fn default_format() -> String {
    "markdown".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The scored observations. By default paste one 'score,label' pair per line, e.g. '0.91,1\\n0.62,0\\n0.44,1' — the score is any continuous number (a probability, a risk score, a biomarker level) and the label is the observed class. Columns may be separated by commas, tabs, semicolons, pipes or spaces, a trailing '%' on a score is read as a percentage, and a non-numeric first row is treated as a header. When the labels are pasted into the separate 'labels' field, this holds the scores column alone. At least 2 observations, at most 20000.",
        ))
        .param(Param::string("labels").describe(
            "Optional second column: one class label per score, in the same order as data, separated by newlines, commas, tabs, semicolons, pipes or spaces (e.g. '1,0,1,1,0' or 'case\\ncontrol\\ncase'). Fill this when the scores and the labels were copied out of two spreadsheet columns; leave it empty to paste 'score,label' pairs in data instead.",
        ))
        .param(
            Param::enumv("input_format", ["auto", "pairs", "columns"])
                .default("auto")
                .describe(
                    "How the input is shaped. 'auto' (default) reads data as 'score,label' pairs when the labels field is empty and as a scores-only column when it is filled; 'pairs' forces two columns inside data; 'columns' forces the scores/labels split across the two fields and fails if labels is empty.",
                ),
        )
        .param(
            Param::enumv("column_order", ["auto", "score_label", "label_score"])
                .default("auto")
                .describe(
                    "Which column of a pasted pair is the score. 'auto' (default) treats a 0/1 column as the class column and the other as the score, falling back to whichever column is numeric; 'score_label' forces score first, 'label_score' forces label first. Only used when the input is 'score,label' pairs.",
                ),
        )
        .param(
            Param::enumv(
                "separator",
                ["auto", "comma", "tab", "semicolon", "pipe", "space", "newline"],
            )
            .default("auto")
            .describe(
                "How each row is split into fields. 'auto' (default) picks the delimiter that splits every row identically, which handles a CSV or spreadsheet paste unchanged. 'newline' means one value per line and is only valid for the two-field 'columns' shape — it cannot split a score from its label on the same row.",
            ),
        )
        .param(
            Param::enumv("header", ["auto", "yes", "no"])
                .default("auto")
                .describe(
                    "Whether the first row holds column names such as 'score,label'. 'auto' (default) drops it only when it is not numeric; 'yes' always drops it; 'no' treats it as data.",
                ),
        )
        .param(Param::string("positive_label").describe(
            "Which label text is the positive class (the event being detected), e.g. '1', 'yes', 'case', 'fraud', 'churn'. Default: auto-detected — a common positive token ('1', 'true', 'yes', 'case', 'disease', 'fraud', 'spam', …) wins over a common negative one ('0', 'no', 'control', 'healthy', 'benign', …), then the larger numeric label, then the later label alphabetically. Set it explicitly when the labels are opaque, or to score one class against all the rest when more than two labels appear.",
        ))
        .param(
            Param::enumv("optimize", ["youden", "f1", "closest", "accuracy", "cost"])
                .default("youden")
                .describe(
                    "Criterion for the recommended cutoff, chosen over every observed score. 'youden' (default) maximizes sensitivity + specificity − 1, the usual ROC optimum; 'f1' maximizes the F1 score, better for rare positives; 'closest' minimizes the distance to the perfect top-left corner; 'accuracy' maximizes the plain hit rate; 'cost' minimizes cost_ratio × false negatives + false positives.",
                ),
        )
        .param(
            Param::number("cost_ratio")
                .min(0.001)
                .max(1000.0)
                .default(1.0)
                .describe(
                    "How many false positives one false negative is worth, used only when optimize='cost' (default 1.0 = equally bad). Raise it when missing a positive is expensive — 10 means one missed case costs as much as ten false alarms, which pushes the cutoff down and catches more positives.",
                ),
        )
        .param(Param::string("threshold").describe(
            "Optional cutoff of your own, on the same scale as the scores (e.g. '0.5'): the report then adds a section with sensitivity, specificity, PPV, NPV, accuracy, F1 and the confusion counts at that exact cutoff, predicting positive when score >= threshold. Default: empty, only the criterion-chosen cutoff is reported.",
        ))
        .param(
            Param::enumv("confidence_level", ["90", "95", "99"])
                .default("95")
                .describe(
                    "Confidence level in percent for the DeLong interval around the AUC (default 95). The interval is AUC ± z × the DeLong standard error, clamped to 0…1; the z-test against AUC = 0.5 and its p-value are unaffected by this choice.",
                ),
        )
        .param(
            Param::integer("table_rows")
                .min(0.0)
                .max(200.0)
                .default(12)
                .describe(
                    "How many rows of the threshold sweep to print, 0 to 200 (default 12). Candidate cutoffs are the distinct observed scores; when there are more than this, rows are sampled evenly across the curve and the chosen cutoff is always kept and marked. 0 hides the table.",
                ),
        )
        .param(Param::boolean("plot").default(true).describe(
            "Draw a fixed-width ASCII ROC curve with the chance diagonal and the chosen cutoff marked (default true). Turn it off for a compact numbers-only report; it is omitted from the 'csv' and 'json' formats either way.",
        ))
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(10.0)
                .default(4)
                .describe(
                    "Decimal places for the reported figures, 0 to 10 (default 4). Only the output is rounded — the AUC, standard error and every rate are computed at full f64 precision, and p-values keep at least 4 decimals.",
                ),
        )
        .param(Param::boolean("percent").default(false).describe(
            "Print rates that live in 0…1 — sensitivity, specificity, FPR, PPV, NPV, accuracy, F1 — as percentages (default false). The AUC, Gini, Youden's J, MCC, Brier score and the thresholds themselves stay plain numbers.",
        ))
        .param(
            Param::enumv("format", ["markdown", "text", "csv", "json"])
                .default("markdown")
                .describe(
                    "Output format: 'markdown' (default) = pipe tables for the summary, the chosen cutoff and the sweep; 'text' = the same content as aligned plain text; 'csv' = a section,metric,value block followed by the threshold table, ready for a spreadsheet; 'json' = the full result including the AUC, its trapezoidal cross-check, the standard error, interval bounds, z, p-value, Gini, Brier score and every operating point.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/roc-auc-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute ROC AUC, a DeLong confidence interval and the optimal cutoff from scored observations",
    skill(
        description = "Score a binary classifier from continuous scores plus observed class labels — paste one 'score,label' row per observation (comma, tab, semicolon, pipe or space separated, header row tolerated), or put the scores and the labels in two separate fields. Returns the Mann-Whitney AUC with ties counted as half (cross-checked against the trapezoidal area under the curve), the DeLong standard error, a 90/95/99% confidence interval, a z-test and two-sided p-value against AUC = 0.5, the Gini coefficient, a Brier score when the scores are probabilities, and a plain-language reading of the AUC. Sweeps every observed cutoff and recommends one by Youden's J, F1, closest-to-top-left, accuracy or a cost ratio of false negatives to false positives, reporting sensitivity, specificity, FPR, PPV, NPV, accuracy, F1, MCC and the TP/FP/TN/FN counts there — plus at any threshold you name. Labels may be text ('case'/'control', 'yes'/'no'); the positive class is auto-detected or set with positive_label. Options: an ASCII ROC plot, a capped threshold table, percent-formatted rates, decimal places, and markdown, text, CSV or JSON output. Binary problems only, up to 20000 observations. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "roc-auc-calculator", |a: Args| {
            gizza_ai_roc_auc_calculator_core::run(
                &a.data,
                &a.labels,
                &a.input_format,
                &a.column_order,
                &a.separator,
                &a.header,
                &a.positive_label,
                &a.optimize,
                a.cost_ratio,
                &a.threshold,
                &a.confidence_level,
                a.table_rows,
                a.plot,
                a.decimals,
                a.percent,
                &a.format,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
