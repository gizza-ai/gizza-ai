//! gizza-ai/html-form-field-extractor core — enumerate every `<form>` in a
//! chunk of HTML and list every form control it contains, with the control's
//! name, id, type, label, required flag, default value, and validation
//! attributes.
//!
//! Parsing uses `scraper` (html5ever, wasm32-safe — the same engine
//! `html-extract` and `html-table-extractor` use) rather than regex: real-world
//! forms carry unquoted attributes, implied closing tags, and sloppy nesting.
//!
//! No wafer / wasm-bindgen deps here — this crate is pure logic so both the
//! block and the browser wrapper can share it.

use scraper::{ElementRef, Html, Selector};
use serde_json::{json, Map, Value};

/// Hard cap on how many form controls a single run will report. Keeps the
/// 64 MiB wasm sandbox comfortable and keeps a runaway paste from producing a
/// megabyte of output.
pub const MAX_FIELDS: usize = 2000;

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Markdown,
    Csv,
}

pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "json" => Ok(Format::Json),
        "markdown" | "md" => Ok(Format::Markdown),
        "csv" => Ok(Format::Csv),
        other => Err(format!(
            "format {other:?} not supported (expected json, markdown, or csv)"
        )),
    }
}

/// Run-time switches, mirrored 1:1 by the block descriptor and the page fields.
#[derive(Debug, Clone)]
pub struct Options {
    /// 0-based form to report; `-1` reports every form.
    pub form_index: i64,
    /// Include submit/reset/button/image controls (default: off — not data fields).
    pub include_buttons: bool,
    /// Include `<input type="hidden">` (default: on — CSRF tokens matter to an audit).
    pub include_hidden: bool,
    /// Resolve each control's visible label text (default: on).
    pub include_labels: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            form_index: -1,
            include_buttons: false,
            include_hidden: true,
            include_labels: true,
        }
    }
}

/// One `<option>` of a `<select>`.
#[derive(Debug, Clone)]
pub struct Opt {
    pub value: String,
    pub label: String,
    pub selected: bool,
    /// Enclosing `<optgroup label="…">`, empty when there is none.
    pub group: String,
}

/// The validation-related attributes of a control. Empty strings mean "absent".
#[derive(Debug, Clone, Default)]
pub struct Validation {
    pub pattern: String,
    pub min: String,
    pub max: String,
    pub step: String,
    pub minlength: String,
    pub maxlength: String,
    pub accept: String,
    pub autocomplete: String,
}

impl Validation {
    /// `(name, value)` pairs for the attributes that are actually present.
    pub fn present(&self) -> Vec<(&'static str, &str)> {
        let all: [(&'static str, &str); 8] = [
            ("pattern", &self.pattern),
            ("min", &self.min),
            ("max", &self.max),
            ("step", &self.step),
            ("minlength", &self.minlength),
            ("maxlength", &self.maxlength),
            ("accept", &self.accept),
            ("autocomplete", &self.autocomplete),
        ];
        all.into_iter().filter(|(_, v)| !v.is_empty()).collect()
    }
}

/// One form control.
#[derive(Debug, Clone)]
pub struct Field {
    /// Source element: `input`, `select`, `textarea`, or `button`.
    pub tag: String,
    /// Effective type: the `type` attribute (defaulted per spec) for inputs and
    /// buttons, or the tag name for `select` / `textarea`.
    pub field_type: String,
    pub name: String,
    pub id: String,
    pub label: String,
    pub required: bool,
    /// The control's initial value (`value`, a textarea's text, or the selected options).
    pub default: String,
    pub placeholder: String,
    pub disabled: bool,
    pub readonly: bool,
    pub checked: bool,
    pub multiple: bool,
    /// A `form="…"` attribute claiming membership in a form elsewhere in the document.
    pub form_attr: String,
    pub validation: Validation,
    pub options: Vec<Opt>,
}

/// One `<form>` (or the trailing pseudo-form holding controls that sit outside
/// every form).
#[derive(Debug, Clone)]
pub struct FormInfo {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub action: String,
    pub method: String,
    pub enctype: String,
    /// True for the synthetic group of controls not inside any `<form>`.
    pub unattached: bool,
    pub fields: Vec<Field>,
}

// ---------------------------------------------------------------- helpers

/// Collapse runs of whitespace (incl. newlines) to single spaces and trim.
fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Trimmed attribute value, empty when absent.
fn attr(el: &ElementRef, name: &str) -> String {
    el.value().attr(name).unwrap_or("").trim().to_string()
}

/// Raw attribute value (no trimming — a default value's spacing is data).
fn attr_raw(el: &ElementRef, name: &str) -> String {
    el.value().attr(name).unwrap_or("").to_string()
}

/// Boolean HTML attribute: present at all = true.
fn flag(el: &ElementRef, name: &str) -> bool {
    el.value().attr(name).is_some()
}

fn tag_name(el: &ElementRef) -> String {
    el.value().name().to_ascii_lowercase()
}

/// A `<textarea>`'s default value: its text, minus one leading newline (HTML
/// parsers drop a newline immediately after the open tag).
fn textarea_default(el: &ElementRef) -> String {
    let t: String = el.text().collect();
    if let Some(r) = t.strip_prefix("\r\n") {
        r.to_string()
    } else if let Some(r) = t.strip_prefix('\n') {
        r.to_string()
    } else {
        t
    }
}

/// Is this control a button rather than a data field?
fn is_button(tag: &str, field_type: &str) -> bool {
    tag == "button" || matches!(field_type, "submit" | "reset" | "button" | "image")
}

fn sel(s: &str) -> Result<Selector, String> {
    Selector::parse(s).map_err(|e| format!("internal selector error for {s:?}: {e}"))
}

// ---------------------------------------------------------------- analysis

/// Parse `html` and return one entry per form (plus a trailing pseudo-form for
/// controls outside every form), honoring `opts`.
pub fn analyze(html: &str, opts: &Options) -> Result<Vec<FormInfo>, String> {
    if html.trim().is_empty() {
        return Err("input HTML is empty — paste the markup containing the form".into());
    }
    let doc = Html::parse_fragment(html);

    let form_sel = sel("form")?;
    let ctrl_sel = sel("input, select, textarea, button")?;
    let label_for_sel = sel("label[for]")?;
    let option_sel = sel("option")?;

    let forms: Vec<ElementRef> = doc.select(&form_sel).collect();

    // id -> visible text of the first `<label for="id">` that points at it.
    let mut labels_by_for: Vec<(String, String)> = Vec::new();
    if opts.include_labels {
        for l in doc.select(&label_for_sel) {
            let key = attr(&l, "for");
            if key.is_empty() || labels_by_for.iter().any(|(k, _)| *k == key) {
                continue;
            }
            labels_by_for.push((key, collapse(&l.text().collect::<String>())));
        }
    }

    let mut buckets: Vec<Vec<Field>> = vec![Vec::new(); forms.len() + 1];
    let mut kept = 0usize;
    let mut excluded = 0usize;

    for ctrl in doc.select(&ctrl_sel) {
        let tag = tag_name(&ctrl);
        let field_type = match tag.as_str() {
            "input" => {
                let t = attr(&ctrl, "type").to_ascii_lowercase();
                if t.is_empty() {
                    "text".to_string()
                } else {
                    t
                }
            }
            "button" => {
                let t = attr(&ctrl, "type").to_ascii_lowercase();
                if t.is_empty() {
                    "submit".to_string()
                } else {
                    t
                }
            }
            other => other.to_string(),
        };

        if !opts.include_buttons && is_button(&tag, &field_type) {
            excluded += 1;
            continue;
        }
        if !opts.include_hidden && field_type == "hidden" {
            excluded += 1;
            continue;
        }

        kept += 1;
        if kept > MAX_FIELDS {
            return Err(format!(
                "too many form fields: more than {MAX_FIELDS} matched — extract one form at a time (set a form index) or split the HTML"
            ));
        }

        let id = attr(&ctrl, "id");
        let label = if opts.include_labels {
            resolve_label(&ctrl, &id, &labels_by_for)
        } else {
            String::new()
        };

        let mut options = Vec::new();
        let default = match tag.as_str() {
            "textarea" => textarea_default(&ctrl),
            "select" => {
                options = collect_options(&ctrl, &option_sel);
                select_default(&options, flag(&ctrl, "multiple"))
            }
            _ => attr_raw(&ctrl, "value"),
        };

        buckets[owning_form(&ctrl, &forms).unwrap_or(forms.len())].push(Field {
            tag: tag.clone(),
            field_type,
            name: attr(&ctrl, "name"),
            id,
            label,
            required: flag(&ctrl, "required"),
            default,
            placeholder: attr_raw(&ctrl, "placeholder"),
            disabled: flag(&ctrl, "disabled"),
            readonly: flag(&ctrl, "readonly"),
            checked: flag(&ctrl, "checked"),
            multiple: flag(&ctrl, "multiple"),
            form_attr: attr(&ctrl, "form"),
            validation: Validation {
                pattern: attr(&ctrl, "pattern"),
                min: attr(&ctrl, "min"),
                max: attr(&ctrl, "max"),
                step: attr(&ctrl, "step"),
                minlength: attr(&ctrl, "minlength"),
                maxlength: attr(&ctrl, "maxlength"),
                accept: attr(&ctrl, "accept"),
                autocomplete: attr(&ctrl, "autocomplete"),
            },
            options,
        });
    }

    if kept == 0 {
        return Err(if excluded > 0 {
            format!(
                "no form fields matched the current filters ({excluded} control(s) excluded) — turn on \"Include buttons\" or \"Include hidden fields\""
            )
        } else {
            "no form fields found — looked for <input>, <select>, <textarea>, and <button> elements"
                .into()
        });
    }

    let mut out: Vec<FormInfo> = Vec::new();
    for (i, f) in forms.iter().enumerate() {
        let method = attr(f, "method").to_ascii_lowercase();
        out.push(FormInfo {
            index: i,
            id: attr(f, "id"),
            name: attr(f, "name"),
            action: attr(f, "action"),
            method: if method.is_empty() {
                "get".to_string()
            } else {
                method
            },
            enctype: attr(f, "enctype").to_ascii_lowercase(),
            unattached: false,
            fields: std::mem::take(&mut buckets[i]),
        });
    }
    let orphans = std::mem::take(&mut buckets[forms.len()]);
    if !orphans.is_empty() {
        out.push(FormInfo {
            index: forms.len(),
            id: String::new(),
            name: String::new(),
            action: String::new(),
            method: String::new(),
            enctype: String::new(),
            unattached: true,
            fields: orphans,
        });
    }

    if opts.form_index >= 0 {
        let want = opts.form_index as usize;
        let total = out.len();
        let picked = out
            .into_iter()
            .find(|f| f.index == want)
            .ok_or_else(|| {
                format!(
                    "form index {want} out of range — found {total} form group(s), so valid indexes are 0..{} (or -1 for all)",
                    total.saturating_sub(1)
                )
            })?;
        return Ok(vec![picked]);
    }

    // Forms with no controls of their own are still real forms worth reporting
    // when listing everything; drop nothing.
    Ok(out)
}

/// Index of the nearest ancestor `<form>`, or `None` for an unattached control.
fn owning_form(ctrl: &ElementRef, forms: &[ElementRef]) -> Option<usize> {
    for anc in ctrl.ancestors() {
        if let Some(i) = forms.iter().position(|f| f.id() == anc.id()) {
            return Some(i);
        }
    }
    None
}

/// Label text: `<label for=id>`, then a wrapping `<label>`, then button text,
/// then `aria-label`, then `title`.
fn resolve_label(ctrl: &ElementRef, id: &str, labels_by_for: &[(String, String)]) -> String {
    if !id.is_empty() {
        if let Some((_, text)) = labels_by_for.iter().find(|(k, _)| k == id) {
            if !text.is_empty() {
                return text.clone();
            }
        }
    }
    for anc in ctrl.ancestors() {
        if let Some(el) = ElementRef::wrap(anc) {
            if tag_name(&el) == "label" {
                let text = collapse(&el.text().collect::<String>());
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    if tag_name(ctrl) == "button" {
        let text = collapse(&ctrl.text().collect::<String>());
        if !text.is_empty() {
            return text;
        }
    }
    let aria = collapse(&attr(ctrl, "aria-label"));
    if !aria.is_empty() {
        return aria;
    }
    collapse(&attr(ctrl, "title"))
}

fn collect_options(select: &ElementRef, option_sel: &Selector) -> Vec<Opt> {
    let mut out = Vec::new();
    for o in select.select(option_sel) {
        let text = collapse(&o.text().collect::<String>());
        // Per the HTML spec an <option> with no value attribute submits its text.
        let value = match o.value().attr("value") {
            Some(v) => v.to_string(),
            None => text.clone(),
        };
        let label = if text.is_empty() {
            attr(&o, "label")
        } else {
            text
        };
        out.push(Opt {
            value,
            label,
            selected: flag(&o, "selected"),
            group: nearest_optgroup(&o, select),
        });
    }
    out
}

fn nearest_optgroup(opt: &ElementRef, stop_at: &ElementRef) -> String {
    for anc in opt.ancestors() {
        if anc.id() == stop_at.id() {
            break;
        }
        if let Some(el) = ElementRef::wrap(anc) {
            if tag_name(&el) == "optgroup" {
                return attr(&el, "label");
            }
        }
    }
    String::new()
}

/// A `<select>`'s initial value: the explicitly selected options, or — matching
/// browser behavior for a single-choice select — the first option.
fn select_default(options: &[Opt], multiple: bool) -> String {
    let chosen: Vec<&str> = options
        .iter()
        .filter(|o| o.selected)
        .map(|o| o.value.as_str())
        .collect();
    if !chosen.is_empty() {
        return chosen.join(", ");
    }
    if !multiple {
        if let Some(first) = options.first() {
            return first.value.clone();
        }
    }
    String::new()
}

// ---------------------------------------------------------------- rendering

/// Analyze `html` and render the result in `fmt`.
pub fn extract(html: &str, fmt: Format, opts: &Options) -> Result<String, String> {
    let forms = analyze(html, opts)?;
    Ok(match fmt {
        Format::Json => render_json(&forms, opts),
        Format::Markdown => render_markdown(&forms, opts),
        Format::Csv => render_csv(&forms, opts)?,
    })
}

fn render_json(forms: &[FormInfo], opts: &Options) -> String {
    let field_count: usize = forms.iter().map(|f| f.fields.len()).sum();
    let arr: Vec<Value> = forms
        .iter()
        .map(|f| {
            let fields: Vec<Value> = f.fields.iter().map(|x| field_json(x, opts)).collect();
            let mut o = Map::new();
            o.insert("index".into(), json!(f.index));
            if f.unattached {
                o.insert("unattached".into(), json!(true));
            }
            o.insert("id".into(), json!(f.id));
            o.insert("name".into(), json!(f.name));
            o.insert("action".into(), json!(f.action));
            o.insert("method".into(), json!(f.method));
            if !f.enctype.is_empty() {
                o.insert("enctype".into(), json!(f.enctype));
            }
            o.insert("field_count".into(), json!(f.fields.len()));
            o.insert("fields".into(), Value::Array(fields));
            Value::Object(o)
        })
        .collect();

    let root = json!({
        "form_count": forms.len(),
        "field_count": field_count,
        "forms": arr,
    });
    serde_json::to_string_pretty(&root).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn field_json(f: &Field, opts: &Options) -> Value {
    let mut o = Map::new();
    o.insert("tag".into(), json!(f.tag));
    o.insert("type".into(), json!(f.field_type));
    o.insert("name".into(), json!(f.name));
    o.insert("id".into(), json!(f.id));
    if opts.include_labels {
        o.insert("label".into(), json!(f.label));
    }
    o.insert("required".into(), json!(f.required));
    o.insert("default".into(), json!(f.default));
    o.insert("placeholder".into(), json!(f.placeholder));
    if f.disabled {
        o.insert("disabled".into(), json!(true));
    }
    if f.readonly {
        o.insert("readonly".into(), json!(true));
    }
    if f.checked {
        o.insert("checked".into(), json!(true));
    }
    if f.multiple {
        o.insert("multiple".into(), json!(true));
    }
    if !f.form_attr.is_empty() {
        o.insert("form_attr".into(), json!(f.form_attr));
    }

    let mut v = Map::new();
    for (k, val) in f.validation.present() {
        v.insert(k.into(), json!(val));
    }
    o.insert("validation".into(), Value::Object(v));

    if !f.options.is_empty() {
        let opts_json: Vec<Value> = f
            .options
            .iter()
            .map(|x| {
                let mut m = Map::new();
                m.insert("value".into(), json!(x.value));
                m.insert("label".into(), json!(x.label));
                m.insert("selected".into(), json!(x.selected));
                if !x.group.is_empty() {
                    m.insert("group".into(), json!(x.group));
                }
                Value::Object(m)
            })
            .collect();
        o.insert("options".into(), Value::Array(opts_json));
    }
    Value::Object(o)
}

fn md_code(s: &str) -> String {
    let c = collapse(s);
    if c.is_empty() {
        "—".to_string()
    } else {
        format!("`{}`", c.replace('`', "'").replace('|', "\\|"))
    }
}

fn md_text(s: &str) -> String {
    let c = collapse(s);
    if c.is_empty() {
        "—".to_string()
    } else {
        c.replace('|', "\\|")
    }
}

fn render_markdown(forms: &[FormInfo], opts: &Options) -> String {
    let mut out = String::new();
    for (n, f) in forms.iter().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        if f.unattached {
            out.push_str(&format!(
                "## Group {} — controls outside any <form>\n\n",
                f.index
            ));
        } else {
            let action = if f.action.is_empty() {
                "(same page)".to_string()
            } else {
                f.action.clone()
            };
            out.push_str(&format!(
                "## Form {} — {} {}\n\n",
                f.index,
                f.method.to_uppercase(),
                action
            ));
            let mut meta: Vec<String> = Vec::new();
            if !f.id.is_empty() {
                meta.push(format!("id: `{}`", f.id));
            }
            if !f.name.is_empty() {
                meta.push(format!("name: `{}`", f.name));
            }
            if !f.enctype.is_empty() {
                meta.push(format!("enctype: `{}`", f.enctype));
            }
            if !meta.is_empty() {
                out.push_str(&meta.join(" · "));
                out.push_str("\n\n");
            }
        }

        if f.fields.is_empty() {
            out.push_str("_No form controls in this form._\n");
            continue;
        }

        if opts.include_labels {
            out.push_str("| # | Name | Type | Label | Required | Default | Validation |\n");
            out.push_str("|---|------|------|-------|----------|---------|------------|\n");
        } else {
            out.push_str("| # | Name | Type | Required | Default | Validation |\n");
            out.push_str("|---|------|------|----------|---------|------------|\n");
        }

        for (i, x) in f.fields.iter().enumerate() {
            let checks: Vec<String> = x
                .validation
                .present()
                .iter()
                .map(|(k, v)| format!("{k}=`{}`", collapse(v).replace('|', "\\|")))
                .collect();
            let mut extra = checks;
            if x.disabled {
                extra.push("disabled".into());
            }
            if x.readonly {
                extra.push("readonly".into());
            }
            if x.checked {
                extra.push("checked".into());
            }
            if x.multiple {
                extra.push("multiple".into());
            }
            let validation = if extra.is_empty() {
                "—".to_string()
            } else {
                extra.join(", ")
            };
            let req = if x.required { "yes" } else { "no" };
            if opts.include_labels {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} | {} |\n",
                    i + 1,
                    md_code(&x.name),
                    md_code(&x.field_type),
                    md_text(&x.label),
                    req,
                    md_code(&x.default),
                    validation
                ));
            } else {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    i + 1,
                    md_code(&x.name),
                    md_code(&x.field_type),
                    req,
                    md_code(&x.default),
                    validation
                ));
            }
        }

        for x in f.fields.iter().filter(|x| !x.options.is_empty()) {
            out.push_str(&format!("\n{} options:\n", md_code(&x.name)));
            for o in &x.options {
                let mut line = format!("- `{}`", collapse(&o.value).replace('|', "\\|"));
                if !o.label.is_empty() && o.label != o.value {
                    line.push_str(&format!(" — {}", md_text(&o.label)));
                }
                if !o.group.is_empty() {
                    line.push_str(&format!(" (group: {})", md_text(&o.group)));
                }
                if o.selected {
                    line.push_str(" **(selected)**");
                }
                out.push_str(&line);
                out.push('\n');
            }
        }
    }
    out
}

fn render_csv(forms: &[FormInfo], opts: &Options) -> Result<String, String> {
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    let mut header: Vec<&str> = vec![
        "form_index",
        "form_action",
        "form_method",
        "tag",
        "type",
        "name",
        "id",
    ];
    if opts.include_labels {
        header.push("label");
    }
    header.extend_from_slice(&[
        "required",
        "default",
        "placeholder",
        "pattern",
        "min",
        "max",
        "step",
        "minlength",
        "maxlength",
        "accept",
        "autocomplete",
        "options",
        "disabled",
        "readonly",
        "checked",
        "multiple",
        "form_attr",
    ]);
    w.write_record(&header)
        .map_err(|e| format!("csv write error: {e}"))?;

    for f in forms {
        for x in &f.fields {
            let options = x
                .options
                .iter()
                .map(|o| {
                    let mut s = o.value.clone();
                    if !o.label.is_empty() && o.label != o.value {
                        s.push_str(&format!("={}", o.label));
                    }
                    if o.selected {
                        s.push_str(" (selected)");
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join("; ");
            let mut row: Vec<String> = vec![
                f.index.to_string(),
                f.action.clone(),
                f.method.clone(),
                x.tag.clone(),
                x.field_type.clone(),
                x.name.clone(),
                x.id.clone(),
            ];
            if opts.include_labels {
                row.push(x.label.clone());
            }
            row.extend([
                x.required.to_string(),
                collapse(&x.default),
                collapse(&x.placeholder),
                x.validation.pattern.clone(),
                x.validation.min.clone(),
                x.validation.max.clone(),
                x.validation.step.clone(),
                x.validation.minlength.clone(),
                x.validation.maxlength.clone(),
                x.validation.accept.clone(),
                x.validation.autocomplete.clone(),
                options,
                x.disabled.to_string(),
                x.readonly.to_string(),
                x.checked.to_string(),
                x.multiple.to_string(),
                x.form_attr.clone(),
            ]);
            w.write_record(&row)
                .map_err(|e| format!("csv write error: {e}"))?;
        }
    }
    let bytes = w
        .into_inner()
        .map_err(|e| format!("csv flush error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("csv encoding error: {e}"))
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    const SIGNUP: &str = r#"
        <form id="signup" action="/register" method="POST" enctype="multipart/form-data">
          <label for="email">Email address</label>
          <input type="email" id="email" name="email" required placeholder="you@example.com"
                 maxlength="64" autocomplete="email">
          <label>Age <input type="number" name="age" min="18" max="120" step="1" value="18"></label>
          <select name="country">
            <optgroup label="North America">
              <option value="us" selected>United States</option>
              <option value="ca">Canada</option>
            </optgroup>
          </select>
          <textarea name="bio" placeholder="About you">hello</textarea>
          <input type="hidden" name="csrf" value="tok123">
          <button type="submit">Sign up</button>
        </form>
    "#;

    #[test]
    fn happy_path_lists_every_field() {
        let forms = analyze(SIGNUP, &Options::default()).unwrap();
        assert_eq!(forms.len(), 1);
        let f = &forms[0];
        assert_eq!(f.id, "signup");
        assert_eq!(f.action, "/register");
        assert_eq!(f.method, "post", "method is lowercased");
        assert_eq!(f.enctype, "multipart/form-data");
        // buttons excluded by default, hidden kept
        let names: Vec<&str> = f.fields.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["email", "age", "country", "bio", "csrf"]);

        let email = &f.fields[0];
        assert_eq!(email.field_type, "email");
        assert!(email.required);
        assert_eq!(email.label, "Email address", "resolved via <label for>");
        assert_eq!(email.placeholder, "you@example.com");
        assert_eq!(email.validation.maxlength, "64");
        assert_eq!(email.validation.autocomplete, "email");

        let age = &f.fields[1];
        assert_eq!(age.label, "Age", "resolved via wrapping <label>");
        assert_eq!(age.validation.min, "18");
        assert_eq!(age.validation.max, "120");
        assert_eq!(age.default, "18");
        assert!(!age.required);

        let country = &f.fields[2];
        assert_eq!(country.tag, "select");
        assert_eq!(country.field_type, "select");
        assert_eq!(country.default, "us");
        assert_eq!(country.options.len(), 2);
        assert_eq!(country.options[0].label, "United States");
        assert!(country.options[0].selected);
        assert_eq!(country.options[1].group, "North America");

        let bio = &f.fields[3];
        assert_eq!(bio.tag, "textarea");
        assert_eq!(bio.default, "hello");

        assert_eq!(f.fields[4].field_type, "hidden");
    }

    #[test]
    fn empty_html_is_an_error() {
        let err = analyze("   ", &Options::default()).unwrap_err();
        assert!(err.contains("empty"), "got {err}");
    }

    #[test]
    fn html_without_controls_is_an_error() {
        let err = analyze("<p>no forms here</p>", &Options::default()).unwrap_err();
        assert!(err.contains("no form fields found"), "got {err}");
    }

    #[test]
    fn filtered_out_everything_says_which_switch_to_flip() {
        let err = analyze("<form><button>Go</button></form>", &Options::default()).unwrap_err();
        assert!(err.contains("Include buttons"), "got {err}");
    }

    #[test]
    fn include_buttons_and_drop_hidden() {
        let opts = Options {
            include_buttons: true,
            include_hidden: false,
            ..Options::default()
        };
        let forms = analyze(SIGNUP, &opts).unwrap();
        let names: Vec<&str> = forms[0].fields.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["email", "age", "country", "bio", ""]);
        assert_eq!(forms[0].fields[4].tag, "button");
        assert_eq!(forms[0].fields[4].field_type, "submit");
        assert_eq!(forms[0].fields[4].label, "Sign up");
    }

    #[test]
    fn bare_input_defaults_to_text_and_method_defaults_to_get() {
        let forms = analyze("<form><input name=q></form>", &Options::default()).unwrap();
        assert_eq!(forms[0].method, "get");
        assert_eq!(forms[0].action, "");
        assert_eq!(forms[0].fields[0].field_type, "text");
    }

    #[test]
    fn unattached_controls_get_their_own_group() {
        let html =
            r#"<form action="/a"><input name="in_form"></form><input name="loose" form="a">"#;
        let forms = analyze(html, &Options::default()).unwrap();
        assert_eq!(forms.len(), 2);
        assert!(!forms[0].unattached);
        assert!(forms[1].unattached);
        assert_eq!(forms[1].index, 1);
        assert_eq!(forms[1].fields[0].name, "loose");
        assert_eq!(forms[1].fields[0].form_attr, "a");
    }

    #[test]
    fn form_index_picks_one_form_and_rejects_out_of_range() {
        let html =
            r#"<form action="/a"><input name="a"></form><form action="/b"><input name="b"></form>"#;
        let opts = Options {
            form_index: 1,
            ..Options::default()
        };
        let forms = analyze(html, &opts).unwrap();
        assert_eq!(forms.len(), 1);
        assert_eq!(forms[0].action, "/b");

        let bad = Options {
            form_index: 5,
            ..Options::default()
        };
        let err = analyze(html, &bad).unwrap_err();
        assert!(err.contains("out of range"), "got {err}");
    }

    #[test]
    fn select_without_selected_falls_back_to_the_first_option() {
        let html = r#"<form><select name="s"><option value="a">A</option><option value="b">B</option></select>
                      <select name="m" multiple><option value="x">X</option></select></form>"#;
        let forms = analyze(html, &Options::default()).unwrap();
        assert_eq!(forms[0].fields[0].default, "a");
        assert_eq!(
            forms[0].fields[1].default, "",
            "multiple selects nothing by default"
        );
        assert!(forms[0].fields[1].multiple);
    }

    #[test]
    fn option_without_value_attribute_submits_its_text() {
        let html = r#"<form><select name="s"><option>Yes</option></select></form>"#;
        let forms = analyze(html, &Options::default()).unwrap();
        assert_eq!(forms[0].fields[0].options[0].value, "Yes");
        assert_eq!(forms[0].fields[0].default, "Yes");
    }

    #[test]
    fn checkbox_and_radio_state_flags() {
        let html = r#"<form>
            <input type="checkbox" name="tos" value="yes" checked required>
            <input type="radio" name="plan" value="pro" disabled>
            <input type="text" name="ro" value="v" readonly>
        </form>"#;
        let forms = analyze(html, &Options::default()).unwrap();
        assert!(forms[0].fields[0].checked && forms[0].fields[0].required);
        assert!(forms[0].fields[1].disabled);
        assert!(forms[0].fields[2].readonly);
    }

    #[test]
    fn aria_label_is_the_fallback_label() {
        let html =
            r#"<form><input name="q" aria-label="Search site"><input name="t" title="Tip"></form>"#;
        let forms = analyze(html, &Options::default()).unwrap();
        assert_eq!(forms[0].fields[0].label, "Search site");
        assert_eq!(forms[0].fields[1].label, "Tip");
    }

    #[test]
    fn labels_can_be_switched_off() {
        let opts = Options {
            include_labels: false,
            ..Options::default()
        };
        let forms = analyze(SIGNUP, &opts).unwrap();
        assert_eq!(forms[0].fields[0].label, "");
        let v: Value = serde_json::from_str(&render_json(&forms, &opts)).unwrap();
        assert!(
            v["forms"][0]["fields"][0].get("label").is_none(),
            "field label key omitted (option labels still carry their own)"
        );
        let csv = render_csv(&forms, &opts).unwrap();
        assert!(!csv.starts_with("form_index,form_action,form_method,tag,type,name,id,label"));
    }

    #[test]
    fn unquoted_and_malformed_markup_still_parses() {
        // Unquoted attributes and an unclosed <p> — regex-based tools trip here.
        let html = "<form action=/x method=post><p><input type=text name=q required></form>";
        let forms = analyze(html, &Options::default()).unwrap();
        assert_eq!(forms[0].method, "post");
        assert_eq!(forms[0].fields[0].name, "q");
        assert!(forms[0].fields[0].required);
    }

    #[test]
    fn json_output_shape() {
        let out = extract(SIGNUP, Format::Json, &Options::default()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["form_count"], json!(1));
        assert_eq!(v["field_count"], json!(5));
        assert_eq!(v["forms"][0]["method"], json!("post"));
        assert_eq!(v["forms"][0]["fields"][0]["name"], json!("email"));
        assert_eq!(
            v["forms"][0]["fields"][0]["validation"]["maxlength"],
            json!("64")
        );
        assert_eq!(
            v["forms"][0]["fields"][2]["options"][1]["value"],
            json!("ca")
        );
        // absent flags are omitted, not emitted as false
        assert!(v["forms"][0]["fields"][0]["disabled"].is_null());
    }

    #[test]
    fn csv_output_has_a_row_per_field() {
        let out = extract(SIGNUP, Format::Csv, &Options::default()).unwrap();
        let lines: Vec<&str> = out.trim_end().lines().collect();
        assert_eq!(lines.len(), 6, "header + 5 fields");
        assert!(lines[0].starts_with("form_index,form_action,form_method,tag,type,name,id,label,"));
        assert!(
            lines[1].starts_with("0,/register,post,input,email,email,email,Email address,true,")
        );
    }

    #[test]
    fn markdown_output_has_a_table_per_form() {
        let out = extract(SIGNUP, Format::Markdown, &Options::default()).unwrap();
        assert!(out.starts_with("## Form 0 — POST /register\n"));
        assert!(out.contains("id: `signup` · enctype: `multipart/form-data`"));
        assert!(out.contains("| 1 | `email` | `email` | Email address | yes | — |"));
        assert!(out.contains("`country` options:"));
        assert!(out.contains("- `us` — United States (group: North America) **(selected)**"));
    }

    #[test]
    fn markdown_escapes_pipes_in_values() {
        let html = r#"<form><input name="p" pattern="a|b" value="x|y"></form>"#;
        let out = extract(html, Format::Markdown, &Options::default()).unwrap();
        assert!(out.contains("`x\\|y`"), "got {out}");
        assert!(out.contains("pattern=`a\\|b`"), "got {out}");
    }

    #[test]
    fn parse_format_accepts_aliases_and_rejects_junk() {
        assert_eq!(parse_format("").unwrap(), Format::Json);
        assert_eq!(parse_format("MD").unwrap(), Format::Markdown);
        assert_eq!(parse_format(" CSV ").unwrap(), Format::Csv);
        let err = parse_format("yaml").unwrap_err();
        assert!(err.contains("json, markdown, or csv"), "got {err}");
    }

    #[test]
    fn field_cap_is_enforced_at_the_boundary() {
        let at_cap = format!("<form>{}</form>", "<input name=x>".repeat(MAX_FIELDS));
        assert_eq!(
            analyze(&at_cap, &Options::default()).unwrap()[0]
                .fields
                .len(),
            MAX_FIELDS
        );
        let over = format!("<form>{}</form>", "<input name=x>".repeat(MAX_FIELDS + 1));
        let err = analyze(&over, &Options::default()).unwrap_err();
        assert!(err.contains("too many form fields"), "got {err}");
    }
}
