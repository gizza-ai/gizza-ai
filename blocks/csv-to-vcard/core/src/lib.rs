//! csv-to-vcard core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Converts a CSV (with a header row) or a JSON
//! array of contacts into a single `.vcf` file of vCards, mapping common column
//! names (First Name, Email, Mobile Phone, Company, City, …) to the matching
//! vCard properties (FN/N/EMAIL/TEL/ORG/ADR/…). Output is RFC-6350/2426-style
//! text with CRLF line endings and 75-octet line folding, ready for bulk import
//! into iPhone / Android / Google / Outlook contacts.

use serde_json::Value;

/// Hard cap on the number of contacts converted in one call, so a huge paste
/// can't blow up memory. The chat/CLI schema advertises this same bound.
pub const MAX_CONTACTS: usize = 5000;

/// Input parsing mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Auto,
    Csv,
    Json,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<InputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(InputFormat::Auto),
            "csv" => Ok(InputFormat::Csv),
            "json" => Ok(InputFormat::Json),
            other => Err(format!(
                "unknown input_format '{other}' (use auto, csv, or json)"
            )),
        }
    }
}

/// vCard version to emit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V3,
    V4,
}

impl Version {
    pub fn parse(s: &str) -> Result<Version, String> {
        match s.trim() {
            "" | "3" | "3.0" => Ok(Version::V3),
            "4" | "4.0" => Ok(Version::V4),
            other => Err(format!("unknown version '{other}' (use 3.0 or 4.0)")),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Version::V3 => "3.0",
            Version::V4 => "4.0",
        }
    }
}

/// A HOME/WORK/CELL/FAX qualifier inferred from a column name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeQual {
    None,
    Home,
    Work,
    Cell,
    Fax,
    Other,
}

impl TypeQual {
    /// The uppercase `TYPE=` value, or `None` for an unqualified property.
    fn type_param(self) -> Option<&'static str> {
        match self {
            TypeQual::None => None,
            TypeQual::Home => Some("HOME"),
            TypeQual::Work => Some("WORK"),
            TypeQual::Cell => Some("CELL"),
            TypeQual::Fax => Some("FAX"),
            TypeQual::Other => Some("OTHER"),
        }
    }
    /// The lowercase 4.0-style `TYPE=` value.
    fn type_param_lower(self) -> Option<&'static str> {
        match self {
            TypeQual::None => None,
            TypeQual::Home => Some("home"),
            TypeQual::Work => Some("work"),
            TypeQual::Cell => Some("cell"),
            TypeQual::Fax => Some("fax"),
            TypeQual::Other => Some("other"),
        }
    }
}

/// Where a CSV/JSON column maps in the vCard.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    FullName,
    Given,
    Family,
    Middle,
    Prefix,
    Suffix,
    Nickname,
    Org,
    Department,
    Title,
    Email(TypeQual),
    Tel(TypeQual),
    Url,
    Bday,
    Note,
    Gender,
    AdrStreet(TypeQual),
    AdrCity(TypeQual),
    AdrState(TypeQual),
    AdrZip(TypeQual),
    AdrCountry(TypeQual),
    Ignore,
}

/// Collapse a header to lowercase alphanumerics only: "E-mail 2" → "email2",
/// "Job_Title" → "jobtitle". Makes phrase matching independent of whatever
/// punctuation/spacing a source app used.
fn collapse(header: &str) -> String {
    header
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Infer the HOME/WORK/CELL/FAX qualifier for phone-like columns.
fn tel_qual(c: &str) -> TypeQual {
    if c.contains("fax") {
        TypeQual::Fax
    } else if c.contains("mobile") || c.contains("cell") {
        TypeQual::Cell
    } else if c.contains("home") || c.contains("personal") {
        TypeQual::Home
    } else if c.contains("work") || c.contains("business") || c.contains("office") {
        TypeQual::Work
    } else if c.contains("other") {
        TypeQual::Other
    } else {
        TypeQual::None
    }
}

/// Infer the HOME/WORK qualifier for email/address columns.
fn place_qual(c: &str) -> TypeQual {
    if c.contains("home") || c.contains("personal") {
        TypeQual::Home
    } else if c.contains("work") || c.contains("business") || c.contains("office") {
        TypeQual::Work
    } else if c.contains("other") {
        TypeQual::Other
    } else {
        TypeQual::None
    }
}

/// Map a source column header to a vCard field. Order matters: specific name
/// parts are checked before the generic "name" → FN, and URL before address so
/// "web address" doesn't land in the street field.
fn classify(header: &str) -> Field {
    let c = collapse(header);
    let s = c.as_str();

    // Structured-name parts (before the generic FullName rule).
    if s.contains("prefix") || s == "salutation" || s == "honorific" {
        return Field::Prefix;
    }
    if s.contains("suffix") {
        return Field::Suffix;
    }
    if s.contains("middle") {
        return Field::Middle;
    }
    if s.contains("firstname")
        || s.contains("givenname")
        || s == "first"
        || s == "given"
        || s == "forename"
    {
        return Field::Given;
    }
    if s.contains("lastname")
        || s.contains("surname")
        || s.contains("familyname")
        || s == "last"
        || s == "family"
    {
        return Field::Family;
    }
    if s.contains("nickname") || s == "nick" || s == "alias" {
        return Field::Nickname;
    }

    // Organisation / job.
    if s.contains("department") || s == "dept" {
        return Field::Department;
    }
    if s.contains("jobtitle")
        || s == "title"
        || s == "position"
        || s == "role"
        || s == "occupation"
        || s == "designation"
        || s == "jobrole"
    {
        return Field::Title;
    }
    if s.contains("organization")
        || s.contains("organisation")
        || s == "company"
        || s == "companyname"
        || s == "employer"
        || s == "org"
    {
        return Field::Org;
    }

    // Communication.
    if s.contains("email") || s == "mail" || s == "emailaddress" || s == "eaddress" {
        return Field::Email(place_qual(s));
    }
    if s.contains("website")
        || s.contains("webpage")
        || s.contains("webaddress")
        || s == "url"
        || s == "web"
        || s == "homepage"
        || s == "site"
    {
        return Field::Url;
    }
    if s.contains("phone")
        || s.contains("mobile")
        || s.contains("cell")
        || s == "tel"
        || s.contains("telephone")
        || s.contains("fax")
    {
        return Field::Tel(tel_qual(s));
    }

    // Postal address components.
    if s.contains("city") || s == "locality" || s == "town" {
        return Field::AdrCity(place_qual(s));
    }
    if s.contains("state") || s == "province" || s == "region" || s == "county" {
        return Field::AdrState(place_qual(s));
    }
    if s.contains("zip")
        || s.contains("postal")
        || s.contains("postcode")
        || s == "pincode"
        || s == "pin"
    {
        return Field::AdrZip(place_qual(s));
    }
    if s.contains("country") {
        return Field::AdrCountry(place_qual(s));
    }
    if s.contains("street") || s.contains("address") || s == "addr" {
        return Field::AdrStreet(place_qual(s));
    }

    // Misc.
    if s.contains("birthday")
        || s.contains("birth")
        || s == "bday"
        || s == "dob"
        || s == "dateofbirth"
    {
        return Field::Bday;
    }
    if s == "gender" || s == "sex" {
        return Field::Gender;
    }
    if s.contains("note")
        || s.contains("comment")
        || s == "description"
        || s == "remarks"
        || s == "memo"
    {
        return Field::Note;
    }
    if s.contains("name") {
        // Generic "name"/"full name"/"display name"/"contact name" last, so the
        // specific parts above win.
        return Field::FullName;
    }
    Field::Ignore
}

/// Escape one value component per RFC 6350 §3.4 (backslash, newline, comma,
/// semicolon). Structured fields escape each component, then join components
/// with an unescaped `;`.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            ',' => out.push_str("\\,"),
            ';' => out.push_str("\\;"),
            c => out.push(c),
        }
    }
    out
}

/// Fold one logical line to ≤75 octets with CRLF + single-space continuations
/// (RFC 6350 §3.2), never splitting a multibyte char.
fn fold(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut bytes = 0usize;
    let mut started = false;
    for ch in line.chars() {
        let cl = ch.len_utf8();
        if started && bytes + cl > 75 {
            out.push_str("\r\n ");
            bytes = 1; // the leading continuation space
        }
        out.push(ch);
        bytes += cl;
        started = true;
    }
    out
}

/// One postal address, accumulated from its component columns.
#[derive(Default)]
struct Address {
    street: String,
    city: String,
    state: String,
    zip: String,
    country: String,
}

impl Address {
    fn is_empty(&self) -> bool {
        self.street.is_empty()
            && self.city.is_empty()
            && self.state.is_empty()
            && self.zip.is_empty()
            && self.country.is_empty()
    }
}

/// A parsed contact row ready to render.
#[derive(Default)]
struct Contact {
    full_name: String,
    given: String,
    family: String,
    middle: String,
    prefix: String,
    suffix: String,
    nickname: String,
    org: String,
    department: String,
    title: String,
    emails: Vec<(TypeQual, String)>,
    tels: Vec<(TypeQual, String)>,
    urls: Vec<String>,
    addresses: Vec<(TypeQual, Address)>,
    bday: String,
    gender: String,
    note: String,
    any: bool,
}

impl Contact {
    fn addr_mut(&mut self, q: TypeQual) -> &mut Address {
        if let Some(pos) = self.addresses.iter().position(|(qq, _)| *qq == q) {
            &mut self.addresses[pos].1
        } else {
            self.addresses.push((q, Address::default()));
            let last = self.addresses.len() - 1;
            &mut self.addresses[last].1
        }
    }
}

/// Turn a parsed (fields, row) into a Contact using the classified fields.
fn build_contact(fields: &[Field], row: &[String]) -> Contact {
    let mut ct = Contact::default();
    for (i, field) in fields.iter().enumerate() {
        let val = row.get(i).map(|s| s.trim()).unwrap_or("");
        if val.is_empty() || matches!(field, Field::Ignore) {
            continue;
        }
        ct.any = true;
        match *field {
            Field::FullName => ct.full_name = val.to_string(),
            Field::Given => ct.given = val.to_string(),
            Field::Family => ct.family = val.to_string(),
            Field::Middle => ct.middle = val.to_string(),
            Field::Prefix => ct.prefix = val.to_string(),
            Field::Suffix => ct.suffix = val.to_string(),
            Field::Nickname => ct.nickname = val.to_string(),
            Field::Org => ct.org = val.to_string(),
            Field::Department => ct.department = val.to_string(),
            Field::Title => ct.title = val.to_string(),
            Field::Email(q) => ct.emails.push((q, val.to_string())),
            Field::Tel(q) => ct.tels.push((q, val.to_string())),
            Field::Url => ct.urls.push(val.to_string()),
            Field::Bday => ct.bday = val.to_string(),
            Field::Note => ct.note = val.to_string(),
            Field::Gender => ct.gender = val.to_string(),
            Field::AdrStreet(q) => ct.addr_mut(q).street = val.to_string(),
            Field::AdrCity(q) => ct.addr_mut(q).city = val.to_string(),
            Field::AdrState(q) => ct.addr_mut(q).state = val.to_string(),
            Field::AdrZip(q) => ct.addr_mut(q).zip = val.to_string(),
            Field::AdrCountry(q) => ct.addr_mut(q).country = val.to_string(),
            Field::Ignore => {}
        }
    }
    ct
}

/// Compose the FN (formatted display name), always non-empty.
fn formatted_name(ct: &Contact) -> String {
    if !ct.full_name.is_empty() {
        return ct.full_name.clone();
    }
    let parts: Vec<&str> = [&ct.prefix, &ct.given, &ct.middle, &ct.family, &ct.suffix]
        .into_iter()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    if !parts.is_empty() {
        return parts.join(" ");
    }
    if !ct.org.is_empty() {
        return ct.org.clone();
    }
    if let Some((_, e)) = ct.emails.first() {
        return e.clone();
    }
    "Unnamed Contact".to_string()
}

/// Emit a `PROP;TYPE=…:value` line respecting the version's TYPE casing.
fn typed_line(prop: &str, q: TypeQual, value: &str, version: Version) -> String {
    let t = match version {
        Version::V3 => q.type_param(),
        Version::V4 => q.type_param_lower(),
    };
    match t {
        Some(t) => format!("{prop};TYPE={t}:{value}"),
        None => format!("{prop}:{value}"),
    }
}

/// Render one Contact to its BEGIN…END vCard lines (already folded).
fn render_vcard(ct: &Contact, version: Version) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut push = |prop: String| lines.push(fold(&prop));

    push("BEGIN:VCARD".to_string());
    push(format!("VERSION:{}", version.label()));

    // N (structured name) — only if any part is present.
    if !(ct.given.is_empty()
        && ct.family.is_empty()
        && ct.middle.is_empty()
        && ct.prefix.is_empty()
        && ct.suffix.is_empty())
    {
        push(format!(
            "N:{};{};{};{};{}",
            escape(&ct.family),
            escape(&ct.given),
            escape(&ct.middle),
            escape(&ct.prefix),
            escape(&ct.suffix),
        ));
    }

    push(format!("FN:{}", escape(&formatted_name(ct))));

    if !ct.nickname.is_empty() {
        push(format!("NICKNAME:{}", escape(&ct.nickname)));
    }
    if !ct.org.is_empty() || !ct.department.is_empty() {
        if ct.department.is_empty() {
            push(format!("ORG:{}", escape(&ct.org)));
        } else {
            push(format!("ORG:{};{}", escape(&ct.org), escape(&ct.department)));
        }
    }
    if !ct.title.is_empty() {
        push(format!("TITLE:{}", escape(&ct.title)));
    }
    for (q, email) in &ct.emails {
        push(typed_line("EMAIL", *q, &escape(email), version));
    }
    for (q, tel) in &ct.tels {
        push(typed_line("TEL", *q, &escape(tel), version));
    }
    for (q, adr) in &ct.addresses {
        if adr.is_empty() {
            continue;
        }
        // ADR = po-box;extended;street;locality;region;postal-code;country
        let body = format!(
            ";;{};{};{};{};{}",
            escape(&adr.street),
            escape(&adr.city),
            escape(&adr.state),
            escape(&adr.zip),
            escape(&adr.country),
        );
        push(typed_line("ADR", *q, &body, version));
    }
    for url in &ct.urls {
        push(format!("URL:{}", escape(url)));
    }
    if !ct.bday.is_empty() {
        push(format!("BDAY:{}", escape(&ct.bday)));
    }
    if !ct.gender.is_empty() && version == Version::V4 {
        // GENDER is a vCard 4.0 property; omit it for 3.0.
        push(format!("GENDER:{}", escape(&ct.gender)));
    }
    if !ct.note.is_empty() {
        push(format!("NOTE:{}", escape(&ct.note)));
    }
    push("END:VCARD".to_string());
    lines
}

/// Sniff the most likely CSV delimiter from the first non-empty line.
fn sniff_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let candidates = [(b',', ','), (b';', ';'), (b'\t', '\t'), (b'|', '|')];
    let mut best = b',';
    let mut best_count = 0usize;
    for (byte, ch) in candidates {
        let n = line.matches(ch).count();
        if n > best_count {
            best_count = n;
            best = byte;
        }
    }
    best
}

fn delimiter_byte(delimiter: &str, data: &str) -> Result<u8, String> {
    Ok(match delimiter.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => sniff_delimiter(data),
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let bytes = other.as_bytes();
            if bytes.len() == 1 {
                bytes[0]
            } else {
                return Err(format!(
                    "delimiter must be auto/comma/semicolon/tab/pipe (or a single char), got '{other}'"
                ));
            }
        }
    })
}

/// Parse CSV text into (headers, rows).
fn parse_csv(data: &str, delimiter: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let delim = delimiter_byte(delimiter, data)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(data.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("could not read the CSV header row: {e}"))?
        .iter()
        .map(|s| s.to_string())
        .collect();
    if headers.is_empty() {
        return Err("the CSV has no header row (the first row must be column names)".into());
    }
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("could not parse a CSV row: {e}"))?;
        rows.push(rec.iter().map(|s| s.to_string()).collect());
    }
    Ok((headers, rows))
}

fn value_to_cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// Parse a JSON array (or single object) of contacts into (headers, rows). The
/// header list is the union of object keys in first-seen order.
fn parse_json(data: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let v: Value =
        serde_json::from_str(data.trim()).map_err(|e| format!("input is not valid JSON: {e}"))?;
    let objects: Vec<&Value> = match &v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(_) => vec![&v],
        _ => {
            return Err(
                "JSON input must be an array of contact objects (or a single object)".into(),
            )
        }
    };
    let mut headers: Vec<String> = Vec::new();
    for obj in &objects {
        let map = obj.as_object().ok_or_else(|| {
            "each JSON contact must be an object of field:value pairs".to_string()
        })?;
        for k in map.keys() {
            if !headers.iter().any(|h| h == k) {
                headers.push(k.clone());
            }
        }
    }
    let mut rows = Vec::new();
    for obj in &objects {
        let map = obj.as_object().unwrap();
        let row = headers
            .iter()
            .map(|h| map.get(h).map(value_to_cell).unwrap_or_default())
            .collect();
        rows.push(row);
    }
    Ok((headers, rows))
}

/// Convert a CSV/JSON contact list into a single `.vcf` file of vCards.
pub fn to_vcards(
    data: &str,
    input_format: &str,
    delimiter: &str,
    version: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err(
            "no input — paste a CSV (with a header row) or a JSON array of contacts".into(),
        );
    }
    let fmt = InputFormat::parse(input_format)?;
    let ver = Version::parse(version)?;

    let is_json = match fmt {
        InputFormat::Json => true,
        InputFormat::Csv => false,
        InputFormat::Auto => matches!(data.trim_start().chars().next(), Some('[') | Some('{')),
    };

    let (headers, rows) = if is_json {
        parse_json(data)?
    } else {
        parse_csv(data, delimiter)?
    };

    if rows.is_empty() {
        return Err("no contact rows found (the input has a header row but no data rows)".into());
    }
    if rows.len() > MAX_CONTACTS {
        return Err(format!(
            "too many contacts: {} (max {MAX_CONTACTS} per conversion)",
            rows.len()
        ));
    }

    let fields: Vec<Field> = headers.iter().map(|h| classify(h)).collect();
    if fields.iter().all(|f| matches!(f, Field::Ignore)) {
        return Err(
            "no recognizable contact columns — expected headers like Name, Email, Phone, Company, City (check the header row)".into(),
        );
    }

    let mut lines: Vec<String> = Vec::new();
    for row in &rows {
        let ct = build_contact(&fields, row);
        if !ct.any {
            continue; // skip an entirely blank row
        }
        lines.extend(render_vcard(&ct, ver));
    }
    if lines.is_empty() {
        return Err("no contact rows found (every data row was blank)".into());
    }
    Ok(lines.join("\r\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_basic_v3() {
        let out = to_vcards(
            "First Name,Last Name,Email,Mobile Phone,Company\nJohn,Doe,john@ex.com,555-1234,Acme",
            "auto",
            "auto",
            "3.0",
        )
        .unwrap();
        let expected = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:Doe;John;;;\r\nFN:John Doe\r\nORG:Acme\r\nEMAIL:john@ex.com\r\nTEL;TYPE=CELL:555-1234\r\nEND:VCARD";
        assert_eq!(out, expected);
    }

    #[test]
    fn full_name_column_and_types() {
        let out = to_vcards(
            "Name,Work Email,Home Phone,Job Title\nJane Smith,jane@work.com,555-9999,CEO",
            "csv",
            "comma",
            "3.0",
        )
        .unwrap();
        assert!(out.contains("FN:Jane Smith"));
        assert!(!out.contains("\r\nN:")); // no structured parts → no N line
        assert!(out.contains("EMAIL;TYPE=WORK:jane@work.com"));
        assert!(out.contains("TEL;TYPE=HOME:555-9999"));
        assert!(out.contains("TITLE:CEO"));
    }

    #[test]
    fn json_input_and_address() {
        let out = to_vcards(
            r#"[{"first name":"Al","last name":"Vy","city":"NYC","state":"NY","zip":"10001"}]"#,
            "auto",
            "auto",
            "3.0",
        )
        .unwrap();
        assert!(out.contains("FN:Al Vy"));
        assert!(out.contains("ADR:;;;NYC;NY;10001;"));
    }

    #[test]
    fn v4_lowercase_types_and_gender() {
        let out = to_vcards("Name,Cell,Gender\nBo,555-0000,M", "csv", "comma", "4.0").unwrap();
        assert!(out.contains("VERSION:4.0"));
        assert!(out.contains("TEL;TYPE=cell:555-0000"));
        assert!(out.contains("GENDER:M"));
    }

    #[test]
    fn gender_dropped_in_v3() {
        let out = to_vcards("Name,Gender\nBo,M", "csv", "comma", "3.0").unwrap();
        assert!(!out.contains("GENDER"));
    }

    #[test]
    fn escaping_special_chars() {
        let out = to_vcards(
            "Name,Company,Note\nAmy,\"Smith, Jones & Co\",\"a; b\"",
            "csv",
            "comma",
            "3.0",
        )
        .unwrap();
        assert!(out.contains("ORG:Smith\\, Jones & Co"));
        assert!(out.contains("NOTE:a\\; b"));
    }

    #[test]
    fn semicolon_delimiter_and_multi_contact() {
        let out =
            to_vcards("name;email\nAl;a@x.com\nBo;b@x.com", "csv", "semicolon", "3.0").unwrap();
        assert_eq!(out.matches("BEGIN:VCARD").count(), 2);
        assert!(out.contains("FN:Al"));
        assert!(out.contains("FN:Bo"));
    }

    #[test]
    fn line_folding_over_75_octets() {
        let long = "x".repeat(120);
        let out = to_vcards(&format!("Name,Note\nA,{long}"), "csv", "comma", "3.0").unwrap();
        assert!(out.contains("\r\n "), "long NOTE should be folded");
        for line in out.split("\r\n") {
            assert!(line.len() <= 75, "line exceeds 75 octets: {line:?}");
        }
    }

    #[test]
    fn err_empty_input() {
        assert!(to_vcards("", "auto", "auto", "3.0").is_err());
    }

    #[test]
    fn err_no_recognizable_columns() {
        let e = to_vcards("foo,bar\n1,2", "csv", "comma", "3.0").unwrap_err();
        assert!(e.contains("recognizable"), "got: {e}");
    }

    #[test]
    fn err_json_not_objects() {
        let e = to_vcards("[1,2,3]", "json", "auto", "3.0").unwrap_err();
        assert!(e.contains("object"), "got: {e}");
    }

    #[test]
    fn err_bad_version() {
        assert!(to_vcards("Name\nA", "csv", "comma", "5.0").is_err());
    }

    #[test]
    fn fn_fallback_to_org_then_email() {
        let out = to_vcards("Company,Email\nAcme,info@acme.com", "csv", "comma", "3.0").unwrap();
        assert!(out.contains("FN:Acme"));
        let out2 = to_vcards("Email\nx@y.com", "csv", "comma", "3.0").unwrap();
        assert!(out2.contains("FN:x@y.com"));
    }
}
