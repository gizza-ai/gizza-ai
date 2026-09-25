//! gizza-ai/confusion-matrix-comparator — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Diffs two
//! confusion matrices entrywise and reports per-class deltas in precision,
//! recall and F-score, plus overall metric triples, the biggest movers and an
//! unpaired test on the accuracy difference. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    matrix_a: String,
    matrix_b: String,
    #[serde(default)]
    labels: String,
    #[serde(default = "default_name_a")]
    name_a: String,
    #[serde(default = "default_name_b")]
    name_b: String,
    #[serde(default = "default_auto")]
    input_format: String,
    #[serde(default = "default_orientation")]
    orientation: String,
    #[serde(default = "default_auto")]
    separator: String,
    #[serde(default = "default_auto")]
    header: String,
    #[serde(default = "default_beta")]
    beta: f64,
    #[serde(default = "default_sort")]
    sort_by: String,
    #[serde(default = "default_true")]
    significance: bool,
    #[serde(default = "default_confidence")]
    confidence_level: String,
    #[serde(default = "default_true")]
    matrix_delta: bool,
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
fn default_name_a() -> String {
    "Model A".into()
}
fn default_name_b() -> String {
    "Model B".into()
}
fn default_orientation() -> String {
    "actual_rows".into()
}
fn default_beta() -> f64 {
    1.0
}
fn default_sort() -> String {
    "class".into()
}
fn default_true() -> bool {
    true
}
fn default_confidence() -> String {
    "95".into()
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
        .param(Param::string("matrix_a").required().describe(
            "The baseline confusion matrix. By default paste a square grid of counts, one row per actual class, e.g. '40,10\\n5,45' — rows are the true class, columns are what the model predicted, and the diagonal is correct predictions. Cells may be separated by commas, tabs, semicolons, pipes or spaces; a header row of class names and a leading column of row labels are both read as class names. Instead of a grid you can paste two columns of 'actual,predicted' labels (one observation per row) or three columns of 'actual,predicted,count'. Counts must be whole numbers — a row-normalised matrix cannot be compared. At most 50 classes.",
        ))
        .param(Param::string("matrix_b").required().describe(
            "The candidate confusion matrix to compare against the baseline, in any of the same shapes, e.g. '45,5\\n8,42'. It must cover the same classes; if it carries class names in a different order the rows and columns are reordered onto the baseline's order before anything is compared. The two matrices do NOT need the same number of observations — every rate is compared directly and the counts are reported side by side.",
        ))
        .param(Param::string("labels").describe(
            "Optional class names, in the order you want the report to use, separated by newlines, commas, tabs, semicolons or pipes (e.g. 'cat,dog,fox' or 'negative,positive'). Fixes the class order for both matrices, renames unlabelled ones, and — for two classes — decides which class is the positive one (the last name). Default: the names pasted with the matrices, the labels found in an 'actual,predicted' list, or 0, 1, 2, … positionally.",
        ))
        .param(Param::string("name_a").default("Model A").describe(
            "Display name for the baseline in every table header and heading, e.g. 'v1', 'Baseline', 'logistic regression'. Default: 'Model A'.",
        ))
        .param(Param::string("name_b").default("Model B").describe(
            "Display name for the candidate, e.g. 'v2', 'Candidate', 'gradient boosting'. Default: 'Model B'. Every delta in the report is the candidate minus the baseline, so a positive number always means the candidate is better on that metric.",
        ))
        .param(
            Param::enumv("input_format", ["auto", "matrix", "labels", "table"])
                .default("auto")
                .describe(
                    "How both inputs are shaped. 'auto' (default) reads a square grid of numbers as a matrix, two columns as 'actual,predicted' observations, and three columns ending in a number as 'actual,predicted,count' triples. 'matrix' forces a K×K grid of counts, 'labels' forces one observation per row, 'table' forces the tallied triples — set it explicitly when an all-numeric paste is ambiguous.",
                ),
        )
        .param(
            Param::enumv("orientation", ["actual_rows", "actual_columns"])
                .default("actual_rows")
                .describe(
                    "Which axis holds the true class. 'actual_rows' (default, the scikit-learn convention) means each row is an actual class and each column a predicted one; 'actual_columns' transposes the grid first — and, for a pasted 'actual,predicted' list or triple table, reads the FIRST column as the prediction instead. Getting this wrong swaps precision with recall, so the report always says which convention it used.",
                ),
        )
        .param(
            Param::enumv(
                "separator",
                ["auto", "comma", "tab", "semicolon", "pipe", "space"],
            )
            .default("auto")
            .describe(
                "How each row is split into fields. 'auto' (default) picks the delimiter that splits every row into the same number of fields, which handles a CSV or spreadsheet paste unchanged; 'space' collapses runs of spaces so a hand-aligned matrix works. Both matrices are read with the same separator.",
            ),
        )
        .param(
            Param::enumv("header", ["auto", "yes", "no"])
                .default("auto")
                .describe(
                    "Whether the first row holds column names rather than counts. 'auto' (default) drops it when it names known columns ('actual', 'predicted', 'count', …) or when it has no numbers and the rest of the grid does; 'yes' always drops row 1 and keeps its cells as class names; 'no' treats row 1 as data.",
                ),
        )
        .param(
            Param::number("beta")
                .min(0.1)
                .max(10.0)
                .default(1.0)
                .describe(
                    "The F-score weight, 0.1 to 10 (default 1.0 = the plain F1, precision and recall weighted equally). Values above 1 weight recall more (2.0 is the usual 'missing a case is worse' choice), values below 1 weight precision more (0.5 for 'a false alarm is worse'). The column headers rename themselves to match, e.g. F2 or F0.5.",
                ),
        )
        .param(
            Param::enumv(
                "sort_by",
                [
                    "class",
                    "f1_delta",
                    "regression",
                    "precision_delta",
                    "recall_delta",
                    "support",
                ],
            )
            .default("class")
            .describe(
                "Row order for the per-class delta table. 'class' (default) keeps the class order. 'f1_delta' puts the biggest F-score gain first, 'regression' puts the biggest LOSS first — the fastest way to see which class a new model broke. 'precision_delta' and 'recall_delta' rank by those gains, 'support' by how many baseline observations each class had.",
            ),
        )
        .param(Param::boolean("significance").default(true).describe(
            "Add a two-proportion z-test on the accuracy difference, with a confidence interval, z, a two-sided p-value and a verdict (default true). It assumes the two matrices come from INDEPENDENT test sets; if both models scored the same items the test is conservative, and the report says so — a paired McNemar test is the right one there and cannot be computed from confusion matrices alone.",
        ))
        .param(
            Param::enumv("confidence_level", ["95", "90", "99"])
                .default("95")
                .describe(
                    "Confidence level in percent for the interval around the accuracy difference (default 95). Also sets the significance threshold: 95 tests at the 5% level, 90 at 10%, 99 at 1%.",
                ),
        )
        .param(Param::boolean("matrix_delta").default(true).describe(
            "Include the entrywise 'candidate minus baseline' grid, one signed count per actual/predicted cell (default true). This is the section that shows WHERE the predictions moved — a diagonal gain paired with an off-diagonal loss in the same row means that class is now being classified correctly.",
        ))
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(10.0)
                .default(4)
                .describe(
                    "Decimal places for every rate and delta, 0 to 10 (default 4). Only the output is rounded — the metrics are computed at full f64 precision, and a p-value always keeps at least 4 decimals so a significant result never prints as 0.",
                ),
        )
        .param(Param::boolean("percent").default(false).describe(
            "Print the rates that live in 0…1 — accuracy, precision, recall, F-score, specificity, balanced accuracy and their deltas — as percentages (default false). Cohen's kappa, the Matthews correlation, z and the p-value stay plain numbers, and counts stay counts.",
        ))
        .param(
            Param::enumv("format", ["markdown", "text", "csv", "json"])
                .default("markdown")
                .describe(
                    "Output format: 'markdown' (default) = pipe tables for the overall, per-class, binary and entrywise sections; 'text' = the same content as aligned plain text; 'csv' = an overall 'section,metric,A,B,delta' block, then one row per class, then one row per matrix cell, ready for a spreadsheet; 'json' = the full result including every metric triple, the per-class array, both matrices, the delta grid and the accuracy test.",
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
    name = "gizza-ai/confusion-matrix-comparator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Diff two confusion matrices entrywise and report per-class deltas in precision, recall and F1",
    skill(
        description = "Compare two confusion matrices — a baseline and a candidate model — and get every metric as a triple (A, B, delta) instead of two reports to diff by eye. Paste each matrix as a square grid of counts (rows = actual class, columns = predicted, header row and row labels read as class names), as two columns of 'actual,predicted' observations, or as three columns of 'actual,predicted,count'; the candidate's rows and columns are reordered onto the baseline's class order automatically. Reports accuracy, balanced accuracy, macro/weighted/micro precision, recall and F-score, Cohen's kappa and the multiclass Matthews correlation, each with its delta; a per-class table of support, precision, recall and F-score for both models with the change in each, sortable by biggest gain or biggest regression; the entrywise 'candidate minus baseline' grid showing where predictions moved; the most improved and most regressed class and the largest cell shifts; a binary block with sensitivity, specificity and the TP/FP/FN/TN counts when there are two classes; and an unpaired two-proportion z-test on the accuracy difference with a confidence interval and p-value (with the paired-McNemar caveat stated, since that test cannot be computed from matrices alone). Options: F-beta weighting, class names and model names, matrix orientation, separator and header handling, percent-formatted rates, decimal places, and markdown, text, CSV or JSON output. Counts must be whole numbers, up to 50 classes. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "confusion-matrix-comparator", |a: Args| {
            gizza_ai_confusion_matrix_comparator_core::run(
                &a.matrix_a,
                &a.matrix_b,
                &a.labels,
                &a.name_a,
                &a.name_b,
                &a.input_format,
                &a.orientation,
                &a.separator,
                &a.header,
                a.beta,
                &a.sort_by,
                a.significance,
                &a.confidence_level,
                a.matrix_delta,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the LLM-facing chat schema is the contract the chat surface,
    /// the CLI and the page form all read, so it is authored here verbatim and
    /// compared against what `descriptor()` derives.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "matrix_a": { "type": "string", "description": "The baseline confusion matrix. By default paste a square grid of counts, one row per actual class, e.g. '40,10\\n5,45' — rows are the true class, columns are what the model predicted, and the diagonal is correct predictions. Cells may be separated by commas, tabs, semicolons, pipes or spaces; a header row of class names and a leading column of row labels are both read as class names. Instead of a grid you can paste two columns of 'actual,predicted' labels (one observation per row) or three columns of 'actual,predicted,count'. Counts must be whole numbers — a row-normalised matrix cannot be compared. At most 50 classes." },
                    "matrix_b": { "type": "string", "description": "The candidate confusion matrix to compare against the baseline, in any of the same shapes, e.g. '45,5\\n8,42'. It must cover the same classes; if it carries class names in a different order the rows and columns are reordered onto the baseline's order before anything is compared. The two matrices do NOT need the same number of observations — every rate is compared directly and the counts are reported side by side." },
                    "labels": { "type": "string", "description": "Optional class names, in the order you want the report to use, separated by newlines, commas, tabs, semicolons or pipes (e.g. 'cat,dog,fox' or 'negative,positive'). Fixes the class order for both matrices, renames unlabelled ones, and — for two classes — decides which class is the positive one (the last name). Default: the names pasted with the matrices, the labels found in an 'actual,predicted' list, or 0, 1, 2, … positionally." },
                    "name_a": { "type": "string", "default": "Model A", "description": "Display name for the baseline in every table header and heading, e.g. 'v1', 'Baseline', 'logistic regression'. Default: 'Model A'." },
                    "name_b": { "type": "string", "default": "Model B", "description": "Display name for the candidate, e.g. 'v2', 'Candidate', 'gradient boosting'. Default: 'Model B'. Every delta in the report is the candidate minus the baseline, so a positive number always means the candidate is better on that metric." },
                    "input_format": { "type": "string", "enum": ["auto", "matrix", "labels", "table"], "default": "auto", "description": "How both inputs are shaped. 'auto' (default) reads a square grid of numbers as a matrix, two columns as 'actual,predicted' observations, and three columns ending in a number as 'actual,predicted,count' triples. 'matrix' forces a K×K grid of counts, 'labels' forces one observation per row, 'table' forces the tallied triples — set it explicitly when an all-numeric paste is ambiguous." },
                    "orientation": { "type": "string", "enum": ["actual_rows", "actual_columns"], "default": "actual_rows", "description": "Which axis holds the true class. 'actual_rows' (default, the scikit-learn convention) means each row is an actual class and each column a predicted one; 'actual_columns' transposes the grid first — and, for a pasted 'actual,predicted' list or triple table, reads the FIRST column as the prediction instead. Getting this wrong swaps precision with recall, so the report always says which convention it used." },
                    "separator": { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe", "space"], "default": "auto", "description": "How each row is split into fields. 'auto' (default) picks the delimiter that splits every row into the same number of fields, which handles a CSV or spreadsheet paste unchanged; 'space' collapses runs of spaces so a hand-aligned matrix works. Both matrices are read with the same separator." },
                    "header": { "type": "string", "enum": ["auto", "yes", "no"], "default": "auto", "description": "Whether the first row holds column names rather than counts. 'auto' (default) drops it when it names known columns ('actual', 'predicted', 'count', …) or when it has no numbers and the rest of the grid does; 'yes' always drops row 1 and keeps its cells as class names; 'no' treats row 1 as data." },
                    "beta": { "type": "number", "minimum": 0.1, "maximum": 10, "default": 1.0, "description": "The F-score weight, 0.1 to 10 (default 1.0 = the plain F1, precision and recall weighted equally). Values above 1 weight recall more (2.0 is the usual 'missing a case is worse' choice), values below 1 weight precision more (0.5 for 'a false alarm is worse'). The column headers rename themselves to match, e.g. F2 or F0.5." },
                    "sort_by": { "type": "string", "enum": ["class", "f1_delta", "regression", "precision_delta", "recall_delta", "support"], "default": "class", "description": "Row order for the per-class delta table. 'class' (default) keeps the class order. 'f1_delta' puts the biggest F-score gain first, 'regression' puts the biggest LOSS first — the fastest way to see which class a new model broke. 'precision_delta' and 'recall_delta' rank by those gains, 'support' by how many baseline observations each class had." },
                    "significance": { "type": "boolean", "default": true, "description": "Add a two-proportion z-test on the accuracy difference, with a confidence interval, z, a two-sided p-value and a verdict (default true). It assumes the two matrices come from INDEPENDENT test sets; if both models scored the same items the test is conservative, and the report says so — a paired McNemar test is the right one there and cannot be computed from confusion matrices alone." },
                    "confidence_level": { "type": "string", "enum": ["95", "90", "99"], "default": "95", "description": "Confidence level in percent for the interval around the accuracy difference (default 95). Also sets the significance threshold: 95 tests at the 5% level, 90 at 10%, 99 at 1%." },
                    "matrix_delta": { "type": "boolean", "default": true, "description": "Include the entrywise 'candidate minus baseline' grid, one signed count per actual/predicted cell (default true). This is the section that shows WHERE the predictions moved — a diagonal gain paired with an off-diagonal loss in the same row means that class is now being classified correctly." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 10, "default": 4, "description": "Decimal places for every rate and delta, 0 to 10 (default 4). Only the output is rounded — the metrics are computed at full f64 precision, and a p-value always keeps at least 4 decimals so a significant result never prints as 0." },
                    "percent": { "type": "boolean", "default": false, "description": "Print the rates that live in 0…1 — accuracy, precision, recall, F-score, specificity, balanced accuracy and their deltas — as percentages (default false). Cohen's kappa, the Matthews correlation, z and the p-value stay plain numbers, and counts stay counts." },
                    "format": { "type": "string", "enum": ["markdown", "text", "csv", "json"], "default": "markdown", "description": "Output format: 'markdown' (default) = pipe tables for the overall, per-class, binary and entrywise sections; 'text' = the same content as aligned plain text; 'csv' = an overall 'section,metric,A,B,delta' block, then one row per class, then one row per matrix cell, ready for a spreadsheet; 'json' = the full result including every metric triple, the per-class array, both matrices, the delta grid and the accuracy test." }
                },
                "required": ["matrix_a", "matrix_b"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Every param needs a `.describe()` an LLM or CLI user can act on.
    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 17, "17 params");
        for (name, spec) in props {
            let d = spec["description"].as_str().unwrap_or("");
            assert!(d.len() > 60, "`{name}` needs a real description, got `{d}`");
        }
    }
}
