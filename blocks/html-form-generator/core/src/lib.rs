//! html-form-generator core — turn a plain-text description of form fields into
//! accessible, ready-to-paste HTML `<form>` markup. No wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.
//!
//! ## Field syntax
//! One field per non-empty line. Each line is pipe-separated into up to three
//! parts: `Label | type | options`.
//!
//! - **Label** (required): the human label. End it with `*` to mark the field
//!   **required** (e.g. `Email*`). The label also seeds the input `name`/`id`
//!   (slugified, e.g. `Full Name` -> `full-name`).
//! - **type** (optional, default `text`): one of the supported control types
//!   (see [`FieldType`]). Aliases: `email`, `password`/`pass`, `number`/`num`,
//!   `tel`/`phone`, `url`, `date`, `time`, `textarea`/`area`/`multiline`,
//!   `select`/`dropdown`, `radio`, `checkbox`/`check`, `checkboxes`, `text`.
//! - **options** (optional): for `select`/`radio`/`checkboxes` a comma-separated
//!   option list (`Red, Green, Blue`); for the other types it is used as the
//!   input's `placeholder`.
//!
//! Lines whose first non-space char is `#` are ignored (comments).

/// A parsed form field.
struct Field {
    label: String,
    name: String,
    ftype: FieldType,
    required: bool,
    /// Placeholder (text-like inputs) — empty if none.
    placeholder: String,
    /// Options for select/radio/checkboxes.
    options: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum FieldType {
    Text,
    Email,
    Password,
    Number,
    Tel,
    Url,
    Date,
    Time,
    Textarea,
    Select,
    Radio,
    Checkbox,
    Checkboxes,
}

impl FieldType {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => FieldType::Text,
            "email" | "mail" => FieldType::Email,
            "password" | "pass" => FieldType::Password,
            "number" | "num" => FieldType::Number,
            "tel" | "phone" => FieldType::Tel,
            "url" | "link" => FieldType::Url,
            "date" => FieldType::Date,
            "time" => FieldType::Time,
            "textarea" | "area" | "multiline" | "message" => FieldType::Textarea,
            "select" | "dropdown" => FieldType::Select,
            "radio" | "radios" => FieldType::Radio,
            "checkbox" | "check" => FieldType::Checkbox,
            "checkboxes" | "checks" => FieldType::Checkboxes,
            other => {
                return Err(format!(
                    "unknown field type {other:?}: expected one of text, email, password, \
                     number, tel, url, date, time, textarea, select, radio, checkbox, checkboxes"
                ))
            }
        })
    }

    /// The HTML `type=` attribute for the single-`<input>` types.
    fn input_type(self) -> &'static str {
        match self {
            FieldType::Text => "text",
            FieldType::Email => "email",
            FieldType::Password => "password",
            FieldType::Number => "number",
            FieldType::Tel => "tel",
            FieldType::Url => "url",
            FieldType::Date => "date",
            FieldType::Time => "time",
            _ => "text",
        }
    }
}

/// HTTP form method.
#[derive(Clone, Copy)]
enum Method {
    Get,
    Post,
}

impl Method {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "post" => Ok(Method::Post),
            "get" => Ok(Method::Get),
            other => Err(format!("invalid method {other:?}: expected \"get\" or \"post\"")),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Method::Get => "get",
            Method::Post => "post",
        }
    }
}

/// HTML-escape text for use in element content or a double-quoted attribute.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Slugify a label into a safe `name`/`id` token: lowercase, non-alphanumerics
/// collapse to single `-`, trimmed. Falls back to `field` when empty.
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "field".into()
    } else {
        out
    }
}

fn parse_field(line: &str) -> Result<Field, String> {
    let mut parts = line.splitn(3, '|');
    let raw_label = parts.next().unwrap_or("").trim();
    let raw_type = parts.next().unwrap_or("").trim();
    let raw_opts = parts.next().unwrap_or("").trim();

    let mut label = raw_label.to_string();
    let required = label.ends_with('*');
    if required {
        label.pop();
        label = label.trim_end().to_string();
    }
    if label.is_empty() {
        return Err(format!("field line {line:?} has an empty label"));
    }

    let ftype = FieldType::parse(raw_type)?;
    let name = slugify(&label);

    let (placeholder, options) = match ftype {
        FieldType::Select | FieldType::Radio | FieldType::Checkboxes => {
            let options: Vec<String> = raw_opts
                .split(',')
                .map(|o| o.trim().to_string())
                .filter(|o| !o.is_empty())
                .collect();
            if options.is_empty() {
                return Err(format!(
                    "field {label:?} of type select/radio/checkboxes needs a comma-separated \
                     option list after the second '|' (e.g. \"Color | select | Red, Green, Blue\")"
                ));
            }
            (String::new(), options)
        }
        _ => (raw_opts.to_string(), Vec::new()),
    };

    Ok(Field {
        label,
        name,
        ftype,
        required,
        placeholder,
        options,
    })
}

/// Ensure every generated `name`/`id` is unique by suffixing duplicates with `-2`, `-3`, …
fn dedupe_names(fields: &mut [Field]) {
    let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for f in fields.iter_mut() {
        let count = seen.entry(f.name.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            f.name = format!("{}-{}", f.name, count);
        }
    }
}

/// Render the input control(s) for one field.
fn render_control(f: &Field) -> String {
    let req_attr = if f.required { " required" } else { "" };
    let name = esc(&f.name);
    let ph = if f.placeholder.is_empty() {
        String::new()
    } else {
        format!(" placeholder=\"{}\"", esc(&f.placeholder))
    };
    match f.ftype {
        FieldType::Textarea => format!(
            "    <textarea id=\"{name}\" name=\"{name}\" rows=\"4\"{ph}{req_attr}></textarea>"
        ),
        FieldType::Select => {
            let mut s = format!("    <select id=\"{name}\" name=\"{name}\"{req_attr}>\n");
            for opt in &f.options {
                s.push_str(&format!(
                    "      <option value=\"{0}\">{1}</option>\n",
                    esc(opt),
                    esc(opt)
                ));
            }
            s.push_str("    </select>");
            s
        }
        FieldType::Radio => {
            let mut s = String::new();
            for (i, opt) in f.options.iter().enumerate() {
                let oid = format!("{}-{}", f.name, i + 1);
                let req = if f.required && i == 0 { " required" } else { "" };
                s.push_str(&format!(
                    "    <label class=\"choice\"><input type=\"radio\" id=\"{0}\" name=\"{1}\" value=\"{2}\"{3}> {4}</label>\n",
                    esc(&oid), name, esc(opt), req, esc(opt)
                ));
            }
            s.pop(); // trailing newline
            s
        }
        FieldType::Checkboxes => {
            let mut s = String::new();
            for (i, opt) in f.options.iter().enumerate() {
                let oid = format!("{}-{}", f.name, i + 1);
                s.push_str(&format!(
                    "    <label class=\"choice\"><input type=\"checkbox\" id=\"{0}\" name=\"{1}[]\" value=\"{2}\"> {3}</label>\n",
                    esc(&oid), name, esc(opt), esc(opt)
                ));
            }
            s.pop();
            s
        }
        FieldType::Checkbox => format!(
            "    <input type=\"checkbox\" id=\"{name}\" name=\"{name}\" value=\"yes\"{req_attr}>"
        ),
        single => format!(
            "    <input type=\"{0}\" id=\"{name}\" name=\"{name}\"{ph}{req_attr}>",
            single.input_type()
        ),
    }
}

/// Built-in stylesheet, emitted when `styled` is true.
const STYLE: &str = r#"<style>
  .gizza-form { max-width: 32rem; font-family: system-ui, sans-serif; }
  .gizza-form .form-row { margin-bottom: 1rem; }
  .gizza-form label { display: block; font-weight: 600; margin-bottom: .35rem; }
  .gizza-form input:not([type="checkbox"]):not([type="radio"]),
  .gizza-form select,
  .gizza-form textarea {
    width: 100%; padding: .55rem .65rem; font-size: 1rem; box-sizing: border-box;
    border: 1px solid #cbd5e1; border-radius: .4rem; background: #fff;
  }
  .gizza-form .choice { font-weight: 400; display: flex; align-items: center; gap: .4rem; }
  .gizza-form .req { color: #dc2626; }
  .gizza-form button {
    padding: .6rem 1.2rem; font-size: 1rem; font-weight: 600; color: #fff;
    background: #2563eb; border: 0; border-radius: .4rem; cursor: pointer;
  }
  .gizza-form button:hover { background: #1d4ed8; }
</style>
"#;

/// Generate the HTML form markup.
///
/// - `fields`: the field description (see module docs).
/// - `method`: `"get"` | `"post"` (blank → `post`).
/// - `action`: the form `action=` URL (blank → omitted).
/// - `submit_label`: text on the submit button (blank → `Submit`).
/// - `styled`: when true, prepend a `<style>` block and add the `gizza-form` class.
pub fn generate(
    fields: &str,
    method: &str,
    action: &str,
    submit_label: &str,
    styled: bool,
) -> Result<String, String> {
    let method = Method::parse(method)?;

    let mut parsed: Vec<Field> = Vec::new();
    for line in fields.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        parsed.push(parse_field(line)?);
    }
    if parsed.is_empty() {
        return Err("no fields: provide at least one field (one per line)".into());
    }
    dedupe_names(&mut parsed);

    let submit = {
        let s = submit_label.trim();
        if s.is_empty() {
            "Submit"
        } else {
            s
        }
    };

    let form_class = if styled { " class=\"gizza-form\"" } else { "" };
    let action_attr = if action.trim().is_empty() {
        String::new()
    } else {
        format!(" action=\"{}\"", esc(action.trim()))
    };

    let mut out = String::new();
    if styled {
        out.push_str(STYLE);
    }
    out.push_str(&format!(
        "<form{form_class}{action_attr} method=\"{}\">\n",
        method.as_str()
    ));

    for f in &parsed {
        out.push_str("  <div class=\"form-row\">\n");
        let req_mark = if f.required {
            " <span class=\"req\" aria-hidden=\"true\">*</span>"
        } else {
            ""
        };
        match f.ftype {
            FieldType::Checkbox => {
                // checkbox: control first, then its bound label.
                out.push_str(&render_control(f));
                out.push('\n');
                out.push_str(&format!(
                    "    <label for=\"{0}\">{1}{2}</label>\n",
                    esc(&f.name),
                    esc(&f.label),
                    req_mark
                ));
            }
            FieldType::Radio | FieldType::Checkboxes => {
                // group label is a <p> (not bound to a single control), then the choices.
                out.push_str(&format!(
                    "    <p class=\"group-label\">{0}{1}</p>\n",
                    esc(&f.label),
                    req_mark
                ));
                out.push_str(&render_control(f));
                out.push('\n');
            }
            _ => {
                out.push_str(&format!(
                    "    <label for=\"{0}\">{1}{2}</label>\n",
                    esc(&f.name),
                    esc(&f.label),
                    req_mark
                ));
                out.push_str(&render_control(f));
                out.push('\n');
            }
        }
        out.push_str("  </div>\n");
    }

    out.push_str(&format!(
        "  <div class=\"form-row\">\n    <button type=\"submit\">{}</button>\n  </div>\n",
        esc(submit)
    ));
    out.push_str("</form>\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_text_and_email_required() {
        let html = generate("Full Name\nEmail* | email", "post", "", "Submit", false).unwrap();
        assert!(html.contains("<form method=\"post\">"));
        assert!(html.contains("<label for=\"full-name\">Full Name</label>"));
        assert!(html.contains("<input type=\"text\" id=\"full-name\" name=\"full-name\">"));
        assert!(html.contains("<input type=\"email\" id=\"email\" name=\"email\" required>"));
        assert!(html.contains("<span class=\"req\" aria-hidden=\"true\">*</span>"));
        assert!(html.contains("<button type=\"submit\">Submit</button>"));
    }

    #[test]
    fn select_renders_options() {
        let html = generate("Color | select | Red, Green, Blue", "get", "", "", false).unwrap();
        assert!(html.contains("<select id=\"color\" name=\"color\">"));
        assert!(html.contains("<option value=\"Red\">Red</option>"));
        assert!(html.contains("<option value=\"Blue\">Blue</option>"));
        assert!(html.contains(">Submit</button>"));
        assert!(html.contains("method=\"get\""));
    }

    #[test]
    fn radio_group_has_shared_name_and_label_p() {
        let html = generate("Size* | radio | S, M, L", "post", "", "Go", false).unwrap();
        assert!(html.contains("<p class=\"group-label\">Size"));
        assert!(html.contains("type=\"radio\" id=\"size-1\" name=\"size\" value=\"S\" required>"));
        assert!(html.contains("type=\"radio\" id=\"size-2\" name=\"size\" value=\"M\">"));
    }

    #[test]
    fn checkbox_label_after_control() {
        let html = generate("I agree | checkbox", "post", "", "", false).unwrap();
        assert!(html.contains("<input type=\"checkbox\" id=\"i-agree\" name=\"i-agree\" value=\"yes\">"));
        assert!(html.contains("<label for=\"i-agree\">I agree</label>"));
    }

    #[test]
    fn checkboxes_use_array_name() {
        let html = generate("Toppings | checkboxes | Cheese, Ham", "post", "", "", false).unwrap();
        assert!(html.contains("name=\"toppings[]\" value=\"Cheese\""));
        assert!(html.contains("name=\"toppings[]\" value=\"Ham\""));
    }

    #[test]
    fn textarea_and_placeholder() {
        let html = generate("Message | textarea | Your message", "post", "", "", false).unwrap();
        assert!(html.contains(
            "<textarea id=\"message\" name=\"message\" rows=\"4\" placeholder=\"Your message\"></textarea>"
        ));
    }

    #[test]
    fn styled_emits_style_and_class_and_action() {
        let html = generate("Name", "post", "/submit", "Send", true).unwrap();
        assert!(html.starts_with("<style>"));
        assert!(html.contains("<form class=\"gizza-form\" action=\"/submit\" method=\"post\">"));
    }

    #[test]
    fn escapes_html_in_labels_and_options() {
        let html = generate("Name <x> & \"y\"", "post", "", "", false).unwrap();
        assert!(html.contains("Name &lt;x&gt; &amp; &quot;y&quot;"));
        assert!(html.contains("id=\"name-x-y\""));
    }

    #[test]
    fn duplicate_labels_get_unique_names() {
        let html = generate("Name\nName", "post", "", "", false).unwrap();
        assert!(html.contains("id=\"name\""));
        assert!(html.contains("id=\"name-2\""));
    }

    #[test]
    fn comments_and_blank_lines_skipped() {
        let html = generate("# a contact form\n\nEmail | email\n", "post", "", "", false).unwrap();
        assert!(html.contains("id=\"email\""));
        // one field row + the submit row.
        assert_eq!(html.matches("class=\"form-row\"").count(), 2);
    }

    #[test]
    fn errors_on_no_fields() {
        let err = generate("\n# only comments\n", "post", "", "", false).unwrap_err();
        assert!(err.contains("no fields"), "got: {err}");
    }

    #[test]
    fn errors_on_unknown_type() {
        let err = generate("X | wizard", "post", "", "", false).unwrap_err();
        assert!(err.contains("unknown field type"), "got: {err}");
    }

    #[test]
    fn errors_on_select_without_options() {
        let err = generate("Color | select", "post", "", "", false).unwrap_err();
        assert!(err.contains("option list"), "got: {err}");
    }

    #[test]
    fn errors_on_empty_label() {
        let err = generate(" * | text", "post", "", "", false).unwrap_err();
        assert!(err.contains("empty label"), "got: {err}");
    }

    #[test]
    fn errors_on_bad_method() {
        let err = generate("Name", "delete", "", "", false).unwrap_err();
        assert!(err.contains("invalid method"), "got: {err}");
    }
}
