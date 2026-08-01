use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumStyle {
    Compile,
    Strip,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub enum_style: EnumStyle,
    pub remove_comments: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            enum_style: EnumStyle::Compile,
            remove_comments: false,
        }
    }
}

pub fn parse_enum_style(value: &str) -> Result<EnumStyle, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "compile" => Ok(EnumStyle::Compile),
        "strip" => Ok(EnumStyle::Strip),
        other => Err(format!(
            "unknown enum_style '{other}' (expected compile or strip)"
        )),
    }
}

pub fn transpile(input: &str, options: &Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input TypeScript source is empty".into());
    }
    if input.len() > 1_000_000 {
        return Err("input is too large (maximum 1 MB)".into());
    }

    let mut out = input.replace("\r\n", "\n").replace('\r', "\n");
    if options.remove_comments {
        out = remove_comments(&out);
    }
    out = strip_type_only_imports_exports(&out);
    out = strip_interface_and_type_blocks(&out);
    out = strip_declare_lines(&out);
    out = strip_implements(&out);
    out = strip_generics(&out);
    out = strip_access_modifiers(&out);
    out = strip_type_annotations(&out);
    out = match options.enum_style {
        EnumStyle::Compile => compile_enums(&out),
        EnumStyle::Strip => strip_enum_blocks(&out),
    };
    out = strip_assertions(&out);
    out = strip_optional_markers(&out);
    out = cleanup_blank_lines(&out);
    Ok(out.trim().to_string())
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid regex")
}

fn strip_type_only_imports_exports(src: &str) -> String {
    let mut s = re(r"(?m)^\s*import\s+type\s+[^;\n]+;?\s*\n?")
        .replace_all(src, "")
        .to_string();
    s = re(r"(?m)^\s*export\s+type\s+[^;\n]+;?\s*\n?")
        .replace_all(&s, "")
        .to_string();
    s = re(
        r"(?m)^\s*export\s+interface\s+[A-Za-z_$][\w$]*(?:\s+extends\s+[^\{]+)?\s*\{[^}]*\}\s*\n?",
    )
    .replace_all(&s, "")
    .to_string();
    // Mixed imports: import { type Foo, Bar } -> import { Bar }
    s = re(r"\{\s*type\s+([A-Za-z_$][\w$]*)(\s*,\s*)?")
        .replace_all(&s, "{ ")
        .to_string();
    s = re(r",\s*type\s+[A-Za-z_$][\w$]*")
        .replace_all(&s, "")
        .to_string();
    s = re(r"\{\s*,").replace_all(&s, "{").to_string();
    s = re(r",\s*\}").replace_all(&s, " }").to_string();
    s
}

fn strip_interface_and_type_blocks(src: &str) -> String {
    let s = re(
        r"(?ms)^\s*(?:export\s+)?interface\s+[A-Za-z_$][\w$]*(?:\s+extends\s+[^\{]+)?\s*\{.*?\}\s*",
    )
    .replace_all(src, "")
    .to_string();
    re(r"(?m)^\s*(?:export\s+)?type\s+[A-Za-z_$][\w$]*(?:<[^;=\n]+>)?\s*=\s*[^;\n]+;?\s*$")
        .replace_all(&s, "")
        .to_string()
}

fn strip_declare_lines(src: &str) -> String {
    re(r"(?m)^\s*declare\s+[^\n]+\n?")
        .replace_all(src, "")
        .to_string()
}

fn strip_implements(src: &str) -> String {
    re(r"\s+implements\s+[A-Za-z_$][\w$]*(?:\s*,\s*[A-Za-z_$][\w$]*)*")
        .replace_all(src, "")
        .to_string()
}

fn strip_generics(src: &str) -> String {
    let s = re(r"(function\s+[A-Za-z_$][\w$]*|class\s+[A-Za-z_$][\w$]*|interface\s+[A-Za-z_$][\w$]*)\s*<[^>\n]+>").replace_all(src, "$1").to_string();
    re(r"([A-Za-z_$][\w$]*)\s*<\s*[A-Za-z_$][\w$]*(?:\s*,\s*[A-Za-z_$][\w$]*)*\s*>\s*\(")
        .replace_all(&s, "$1(")
        .to_string()
}

fn strip_access_modifiers(src: &str) -> String {
    re(r"\b(?:public|private|protected|readonly|abstract|override|declare)\s+")
        .replace_all(src, "")
        .to_string()
}

fn strip_type_annotations(src: &str) -> String {
    let mut s = src.to_string();
    // Variable declarations: const x: Type = -> const x =
    s = re(r"\b(const|let|var)\s+([A-Za-z_$][\w$]*)[?!]?\s*:\s*([^=;\n]+)([=;\n])")
        .replace_all(&s, "$1 $2$4")
        .to_string();
    // Parameters and object/class fields before comma/paren/equals/semicolon.
    let param_re = re(r"([,(]\s*)([A-Za-z_$][\w$]*)[?!]?\s*:\s*([^,)=\n]+)([,)=])");
    for _ in 0..4 {
        s = param_re.replace_all(&s, "$1$2$4").to_string();
    }
    s = re(r"(?m)^(\s*)([A-Za-z_$][\w$]*)[?!]?\s*:\s*([^;=\n]+)([;=])")
        .replace_all(&s, "$1$2$4")
        .to_string();
    s = re(r"([\{;]\s*)([A-Za-z_$][\w$]*)[?!]?\s*:\s*([^;=\n]+)([;=])")
        .replace_all(&s, "$1$2$4")
        .to_string();
    // Function return types: ): Type { or ): Type =>
    s = re(r"\)\s*:\s*([^\{=;\n]+)\s*(\{|=>)")
        .replace_all(&s, ") $2")
        .to_string();
    // Arrow single-param: (x: T) already handled; catch `x: T =>`.
    re(r"([A-Za-z_$][\w$]*)\s*:\s*([^=\n]+)(=>)")
        .replace_all(&s, "$1 $3")
        .to_string()
}

fn strip_assertions(src: &str) -> String {
    let s = re(r"\s+as\s+(?:const|[A-Za-z_$][\w$]*(?:\s*<[^>]+>)?(?:\[\])?(?:\s*\|\s*[A-Za-z_$][\w$]*(?:\[\])?)*)").replace_all(src, "").to_string();
    re(r"\s+satisfies\s+([^,;\n)]+)([,;\n)])")
        .replace_all(&s, "$2")
        .to_string()
}

fn strip_optional_markers(src: &str) -> String {
    let s = re(r"([A-Za-z_$][\w$]*)[?!](\s*[,:)=;])")
        .replace_all(src, "$1$2")
        .to_string();
    re(r"([A-Za-z_$][\w$]*)!\.")
        .replace_all(&s, "$1.")
        .to_string()
}

fn strip_enum_blocks(src: &str) -> String {
    re(r"(?ms)^\s*(?:export\s+)?(?:const\s+)?enum\s+[A-Za-z_$][\w$]*\s*\{.*?\}\s*")
        .replace_all(src, "")
        .to_string()
}

fn compile_enums(src: &str) -> String {
    let enum_re = re(r"(?ms)(export\s+)?enum\s+([A-Za-z_$][\w$]*)\s*\{(.*?)\}");
    enum_re
        .replace_all(src, |caps: &regex::Captures| {
            let export = caps.get(1).map_or("", |m| m.as_str());
            let name = caps.get(2).unwrap().as_str();
            let body = caps.get(3).unwrap().as_str();
            let mut js = String::new();
            js.push_str(export);
            js.push_str("const ");
            js.push_str(name);
            js.push_str(" = {\n");
            let mut next = 0i64;
            for raw in body.split(',') {
                let item = raw.trim();
                if item.is_empty() {
                    continue;
                }
                let mut parts = item.splitn(2, '=');
                let key = parts
                    .next()
                    .unwrap()
                    .trim()
                    .trim_matches(|c: char| c == '\'' || c == '"');
                if key.is_empty() {
                    continue;
                }
                let value = if let Some(v) = parts.next() {
                    let v = v.trim();
                    if let Ok(n) = v.parse::<i64>() {
                        next = n + 1;
                        n.to_string()
                    } else {
                        v.to_string()
                    }
                } else {
                    let v = next;
                    next += 1;
                    v.to_string()
                };
                js.push_str("  ");
                js.push_str(key);
                js.push_str(": ");
                js.push_str(&value);
                js.push_str(",\n");
            }
            js.push_str("};");
            js
        })
        .to_string()
}

fn remove_comments(src: &str) -> String {
    let s = re(r"(?m)//[^\n]*").replace_all(src, "").to_string();
    re(r"(?ms)/\*.*?\*/").replace_all(&s, "").to_string()
}

fn cleanup_blank_lines(src: &str) -> String {
    re(r"\n{3,}").replace_all(src, "\n\n").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_type_syntax() {
        let ts = r#"
import type { User } from './types';
import { type Config, load } from './cfg';
interface Person { name: string; age?: number }
type Id = string | number;
export function greet<T>(name: string, times?: number): string {
  const label: string = name as string;
  return label;
}
class Box implements Serializable { private value!: number; }
"#;
        let js = transpile(ts, &Options::default()).unwrap();
        assert!(!js.contains("interface Person"));
        assert!(!js.contains("type Id"));
        assert!(!js.contains("import type"));
        assert!(js.contains("import { load } from './cfg';"));
        assert!(js.contains("function greet(name, times)"));
        assert!(js.contains("const label= name;") || js.contains("const label = name;"));
        assert!(js.contains("class Box { value; }"));
    }

    #[test]
    fn compiles_or_strips_enums() {
        let ts = "enum Color { Red, Green = 4, Blue }\nconst c: Color = Color.Blue;";
        let js = transpile(ts, &Options::default()).unwrap();
        assert!(js.contains("const Color ="));
        assert!(js.contains("Red: 0"));
        assert!(js.contains("Green: 4"));
        let stripped = transpile(
            ts,
            &Options {
                enum_style: EnumStyle::Strip,
                remove_comments: false,
            },
        )
        .unwrap();
        assert!(!stripped.contains("enum Color"));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(transpile("   ", &Options::default()).is_err());
    }
}
