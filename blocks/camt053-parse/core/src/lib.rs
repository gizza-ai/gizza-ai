//! camt053-parse core — parse an ISO 20022 CAMT bank statement XML
//! (camt.053 Bank-to-Customer Statement; the sibling camt.052 Account Report
//! and camt.054 Debit/Credit Notification share the structure and are accepted
//! too) into structured statements: account + balances (`Bal`) plus the
//! individual `Ntry` entries and their `NtryDtls/TxDtls` transaction details.
//! Pure compute — no I/O, no wafer/wasm-bindgen deps. Shared by the chat skill
//! block and the web page.
//!
//! CAMT layout (namespaces vary by version — camt.053.001.02 … .001.13 — so all
//! matching is by LOCAL element name):
//!   Document > BkToCstmrStmt > Stmt        (camt.053; camt.052 = Rpt,
//!                                           camt.054 = Ntfctn)
//!   Stmt: Id, ElctrncSeqNb, CreDtTm, Acct (IBAN/Ccy/Ownr), Bal*, Ntry*
//!   Bal:  Tp/CdOrPrtry/Cd (OPBD/CLBD/OPAV/CLAV/…), Amt@Ccy, CdtDbtInd, Dt
//!   Ntry: Amt@Ccy, CdtDbtInd, RvslInd, Sts (or Sts/Cd in v8+), BookgDt, ValDt,
//!         AcctSvcrRef, BkTxCd, NtryDtls > TxDtls* (Refs/EndToEndId, RltdPties
//!         Dbtr|Cdtr (or …/Pty/Nm in v8+), RmtInf/Ustrd, AmtDtls, Chrgs)

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;

/// Output shape: structured JSON or a flat transaction CSV.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Json,
    Csv,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(Output::Json),
            "csv" => Ok(Output::Csv),
            other => Err(format!("unknown output '{other}' (use json or csv)")),
        }
    }
}

/// How dates are rendered in the output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    Iso,
    Us,
    Eu,
    Raw,
}

impl DateFormat {
    pub fn parse(s: &str) -> Result<DateFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "iso" => Ok(DateFormat::Iso),
            "us" => Ok(DateFormat::Us),
            "eu" => Ok(DateFormat::Eu),
            "raw" => Ok(DateFormat::Raw),
            other => Err(format!("unknown date_format '{other}' (use iso, us, eu, or raw)")),
        }
    }
}

/// A parsed `Bal` balance record.
#[derive(Serialize, Clone)]
pub struct Balance {
    /// ISO balance type code: `OPBD`, `CLBD`, `OPAV`, `CLAV`, `PRCD`, `FWAV`,
    /// `ITBD`, `ITAV`, or a bank-proprietary code.
    pub type_code: String,
    /// Human-readable meaning of `type_code` (empty for proprietary codes).
    pub description: String,
    /// Balance date, formatted per `date_format`.
    pub date: String,
    /// ISO-4217 currency code, e.g. `EUR`.
    pub currency: String,
    /// Amount. Signed per `signed_amounts` (a DBIT balance is negative).
    pub amount: f64,
    /// `CRDT` (positive) or `DBIT` (overdrawn).
    pub credit_debit: String,
}

/// One transaction row: an `Ntry`, or one of its `TxDtls` when expanding
/// batch entries.
#[derive(Serialize)]
pub struct Transaction {
    /// `BookgDt` — when the bank booked the entry.
    pub booking_date: String,
    /// `ValDt` — when the movement takes effect for interest.
    pub value_date: String,
    /// `CRDT` (money in) or `DBIT` (money out).
    pub credit_debit: String,
    /// `RvslInd` — true when the entry reverses a previous one.
    pub reversal: bool,
    /// Entry status, e.g. `BOOK` (booked) or `PDNG` (pending).
    pub status: String,
    /// Amount. Signed per `signed_amounts` (DBIT negative when enabled).
    pub amount: f64,
    /// ISO-4217 currency of the amount.
    pub currency: String,
    /// `BkTxCd` as `DOMAIN.FAMILY.SUBFAMILY` (e.g. `PMNT.ICDT.ESCT`), or the
    /// bank-proprietary code when no domain code is present.
    pub bank_transaction_code: String,
    /// `NtryRef` — the entry's own reference, if any.
    pub entry_reference: String,
    /// `AcctSvcrRef` — the bank's (account servicer's) reference.
    pub bank_reference: String,
    /// `Refs/EndToEndId` from the transaction details.
    pub end_to_end_id: String,
    /// The other party: creditor name for DBIT (money out), debtor for CRDT.
    pub counterparty: String,
    /// The other party's account IBAN, when given.
    pub counterparty_iban: String,
    /// `RmtInf/Ustrd` lines joined; falls back to the structured
    /// `Strd/CdtrRefInf/Ref` creditor reference.
    pub remittance_info: String,
    /// `TtlChrgsAndTaxAmt` — total charges on this entry/transaction, if given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charges: Option<f64>,
    /// `AmtDtls/TxAmt/CcyXchg/XchgRate` — FX rate applied, if given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_rate: Option<f64>,
    /// With `expand_details=false`, how many `TxDtls` payments this batch entry
    /// rolls up (only present when more than one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_count: Option<usize>,
}

/// A single statement (`Stmt` — or `Rpt`/`Ntfctn` for camt.052/054).
#[derive(Serialize)]
pub struct Statement {
    /// Which ISO 20022 message carried it: `camt.053`, `camt.052`, or `camt.054`.
    pub message_type: String,
    /// `Id` — the statement identification.
    pub statement_id: String,
    /// `ElctrncSeqNb` (or `LglSeqNb`) — statement sequence number.
    pub sequence_number: String,
    /// `CreDtTm` — when the statement was created, formatted per `date_format`.
    pub creation_date: String,
    /// `Acct/Id/IBAN` (or `Acct/Id/Othr/Id`).
    pub account_iban: String,
    /// `Acct/Ccy`.
    pub account_currency: String,
    /// `Acct/Ownr/Nm`, when given.
    pub account_owner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opening_balance: Option<Balance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closing_balance: Option<Balance>,
    /// Every `Bal` record, in document order (includes the two above).
    pub balances: Vec<Balance>,
    pub transaction_count: usize,
    pub transactions: Vec<Transaction>,
}

/// String-typed entry point shared by the chat block, CLI, and web page. Parses
/// the `output` and `date_format` selectors, then delegates to [`parse`]. This
/// is the single place the three surfaces converge, so option names stay in sync.
///
/// - `output`: `json` (default) or `csv`.
/// - `date_format`: `iso` (default), `us`, `eu`, or `raw` (source string verbatim).
/// - `delimiter`: CSV field separator — `comma` (default), `semicolon`, `tab`,
///   or `pipe`. Only consulted for CSV output.
/// - `signed_amounts`: when true (default), DBIT amounts/balances are negative;
///   when false, amounts stay positive and `credit_debit` carries the direction.
/// - `expand_details`: when true (default), a batch `Ntry` with several
///   `TxDtls` becomes one row per payment; when false, one row per entry.
pub fn run(
    data: &str,
    output: &str,
    date_format: &str,
    delimiter: &str,
    signed_amounts: bool,
    expand_details: bool,
) -> Result<String, String> {
    let out = Output::parse(output)?;
    let fmt = DateFormat::parse(date_format)?;
    parse(data, out, fmt, delimiter, signed_amounts, expand_details)
}

/// Parse `data` (CAMT XML text) and render it as `output` (JSON or CSV).
pub fn parse(
    data: &str,
    output: Output,
    date_format: DateFormat,
    delimiter: &str,
    signed_amounts: bool,
    expand_details: bool,
) -> Result<String, String> {
    let statements = parse_statements(data, date_format, signed_amounts, expand_details)?;
    match output {
        Output::Json => serde_json::to_string_pretty(&statements)
            .map_err(|e| format!("failed to serialize JSON: {e}")),
        Output::Csv => write_csv(&statements, delimiter),
    }
}

// ---------------------------------------------------------------------------
// Minimal namespace-agnostic XML tree
// ---------------------------------------------------------------------------

/// An XML element: local tag name, attributes (local names), children, text.
struct Node {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn new(tag: String) -> Self {
        Node { tag, attrs: Vec::new(), children: Vec::new(), text: String::new() }
    }

    /// First child element with the given local name.
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.tag == name)
    }

    /// All child elements with the given local name.
    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.tag == name)
    }

    /// Walk a child path (`["A","B"]` = first `A` child's first `B` child).
    fn find(&self, path: &[&str]) -> Option<&Node> {
        let mut cur = self;
        for name in path {
            cur = cur.child(name)?;
        }
        Some(cur)
    }

    /// Trimmed text content of this element.
    fn text(&self) -> String {
        self.text.trim().to_string()
    }

    /// Trimmed text of the element at a child path, or empty.
    fn find_text(&self, path: &[&str]) -> String {
        self.find(path).map(|n| n.text()).unwrap_or_default()
    }

    /// Attribute value by local name.
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Depth-first search for the first descendant (or self) with this name.
    fn descendant(&self, name: &str) -> Option<&Node> {
        if self.tag == name {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.descendant(name))
    }
}

/// Strip an `ns:local` prefix, returning the local name.
fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
    }
}

/// Parse XML text into a single root [`Node`] (namespace prefixes stripped).
fn parse_xml(data: &str) -> Result<Node, String> {
    let mut reader = Reader::from_str(data);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(format!(
                    "malformed XML at position {}: {e}",
                    reader.buffer_position()
                ))
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let mut node = Node::new(local_name(e.name().as_ref()));
                read_attrs(&e, &mut node)?;
                stack.push(node);
            }
            Ok(Event::Empty(e)) => {
                let mut node = Node::new(local_name(e.name().as_ref()));
                read_attrs(&e, &mut node)?;
                attach(&mut stack, &mut root, node)?;
            }
            Ok(Event::End(_)) => {
                let node = stack.pop().ok_or_else(|| "unexpected closing tag".to_string())?;
                attach(&mut stack, &mut root, node)?;
            }
            Ok(Event::Text(t)) => {
                let txt = t.unescape().map_err(|e| e.to_string())?;
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&txt);
                }
            }
            Ok(Event::CData(t)) => {
                let txt = String::from_utf8_lossy(&t.into_inner()).into_owned();
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&txt);
                }
            }
            _ => {} // comments, PIs, declarations, doctype
        }
        buf.clear();
    }

    if !stack.is_empty() {
        return Err("malformed XML: unclosed element(s)".into());
    }
    root.ok_or_else(|| "no XML root element found".into())
}

fn attach(stack: &mut Vec<Node>, root: &mut Option<Node>, node: Node) -> Result<(), String> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
        Ok(())
    } else if root.is_none() {
        *root = Some(node);
        Ok(())
    } else {
        Err("multiple root elements: XML must have exactly one root".into())
    }
}

fn read_attrs(e: &quick_xml::events::BytesStart, node: &mut Node) -> Result<(), String> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| err.to_string())?;
        let key = local_name(attr.key.as_ref());
        let val = attr
            .unescape_value()
            .map_err(|err| err.to_string())?
            .into_owned();
        node.attrs.push((key, val));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// CAMT extraction
// ---------------------------------------------------------------------------

/// Parse `data` into structured statements (the shared front half of `parse`).
pub fn parse_statements(
    data: &str,
    date_format: DateFormat,
    signed: bool,
    expand_details: bool,
) -> Result<Vec<Statement>, String> {
    if data.trim().is_empty() {
        return Err("no XML content: the input is empty".into());
    }
    let root = parse_xml(data)?;

    // The three sibling messages share the statement structure.
    const MESSAGES: [(&str, &str, &str); 3] = [
        ("BkToCstmrStmt", "Stmt", "camt.053"),
        ("BkToCstmrAcctRpt", "Rpt", "camt.052"),
        ("BkToCstmrDbtCdtNtfctn", "Ntfctn", "camt.054"),
    ];

    let (msg, stmt_tag, message_type) = MESSAGES
        .iter()
        .find_map(|(wrapper, stmt_tag, mt)| {
            root.descendant(wrapper).map(|m| (m, *stmt_tag, *mt))
        })
        .ok_or_else(|| {
            "not a CAMT bank statement: expected <BkToCstmrStmt> (camt.053), \
             <BkToCstmrAcctRpt> (camt.052) or <BkToCstmrDbtCdtNtfctn> (camt.054) \
             inside the ISO 20022 <Document> root"
                .to_string()
        })?;

    let statements: Vec<Statement> = msg
        .children_named(stmt_tag)
        .map(|st| parse_statement(st, message_type, date_format, signed, expand_details))
        .collect::<Result<_, _>>()?;

    if statements.is_empty() {
        return Err(format!(
            "the message contains no <{stmt_tag}> statements — is the file truncated?"
        ));
    }
    Ok(statements)
}

fn parse_statement(
    st: &Node,
    message_type: &str,
    fmt: DateFormat,
    signed: bool,
    expand_details: bool,
) -> Result<Statement, String> {
    let acct = st.child("Acct");
    let account_iban = acct
        .map(|a| {
            let iban = a.find_text(&["Id", "IBAN"]);
            if iban.is_empty() { a.find_text(&["Id", "Othr", "Id"]) } else { iban }
        })
        .unwrap_or_default();
    let account_currency = acct.map(|a| a.find_text(&["Ccy"])).unwrap_or_default();
    let account_owner = acct
        .map(|a| {
            let nm = a.find_text(&["Ownr", "Nm"]);
            if nm.is_empty() { a.find_text(&["Ownr", "Pty", "Nm"]) } else { nm }
        })
        .unwrap_or_default();

    let mut sequence_number = st.find_text(&["ElctrncSeqNb"]);
    if sequence_number.is_empty() {
        sequence_number = st.find_text(&["LglSeqNb"]);
    }

    let balances: Vec<Balance> = st
        .children_named("Bal")
        .map(|b| parse_balance(b, fmt, signed))
        .collect::<Result<_, _>>()?;
    let pick = |code: &str| balances.iter().find(|b| b.type_code == code).cloned();

    let mut transactions: Vec<Transaction> = Vec::new();
    for ntry in st.children_named("Ntry") {
        parse_entry(ntry, fmt, signed, expand_details, &mut transactions)?;
    }

    Ok(Statement {
        message_type: message_type.to_string(),
        statement_id: st.find_text(&["Id"]),
        sequence_number,
        creation_date: format_date(&st.find_text(&["CreDtTm"]), fmt),
        account_iban,
        account_currency,
        account_owner,
        opening_balance: pick("OPBD"),
        closing_balance: pick("CLBD"),
        balances,
        transaction_count: transactions.len(),
        transactions,
    })
}

/// ISO balance type codes → readable descriptions.
fn balance_description(code: &str) -> &'static str {
    match code {
        "OPBD" => "Opening booked",
        "CLBD" => "Closing booked",
        "OPAV" => "Opening available",
        "CLAV" => "Closing available",
        "PRCD" => "Previously closed booked",
        "FWAV" => "Forward available",
        "ITBD" => "Interim booked",
        "ITAV" => "Interim available",
        _ => "",
    }
}

fn parse_balance(b: &Node, fmt: DateFormat, signed: bool) -> Result<Balance, String> {
    let mut type_code = b.find_text(&["Tp", "CdOrPrtry", "Cd"]);
    if type_code.is_empty() {
        type_code = b.find_text(&["Tp", "CdOrPrtry", "Prtry"]);
    }
    let amt = b
        .child("Amt")
        .ok_or_else(|| format!("balance '{type_code}' is missing its <Amt> amount"))?;
    let amount = parse_amount(&amt.text())
        .ok_or_else(|| format!("malformed balance amount '{}' ({type_code})", amt.text()))?;
    let credit_debit = b.find_text(&["CdtDbtInd"]);
    let amount = apply_sign(amount, &credit_debit, signed);
    // Bal/Dt is a date-or-datetime choice: <Dt><Dt>…</Dt></Dt> or <Dt><DtTm>…</DtTm></Dt>.
    let date = date_choice(b.child("Dt"), fmt);
    Ok(Balance {
        description: balance_description(&type_code).to_string(),
        type_code,
        date,
        currency: amt.attr("Ccy").unwrap_or_default().to_string(),
        amount,
        credit_debit,
    })
}

/// Parse one `Ntry`, appending one row per `TxDtls` (expand) or per entry.
fn parse_entry(
    ntry: &Node,
    fmt: DateFormat,
    signed: bool,
    expand_details: bool,
    out: &mut Vec<Transaction>,
) -> Result<(), String> {
    let amt_node = ntry
        .child("Amt")
        .ok_or_else(|| "malformed <Ntry>: missing its <Amt> amount".to_string())?;
    let entry_amount = parse_amount(&amt_node.text())
        .ok_or_else(|| format!("malformed <Ntry> amount '{}'", amt_node.text()))?;
    let entry_currency = amt_node.attr("Ccy").unwrap_or_default().to_string();
    let entry_cd = ntry.find_text(&["CdtDbtInd"]);
    let reversal = ntry.find_text(&["RvslInd"]).eq_ignore_ascii_case("true");
    // v2 has <Sts>BOOK</Sts>; v8+ wraps it: <Sts><Cd>BOOK</Cd></Sts>.
    let status = match ntry.child("Sts") {
        Some(s) if !s.text().is_empty() => s.text(),
        Some(s) => s.find_text(&["Cd"]),
        None => String::new(),
    };
    let booking_date = date_choice(ntry.child("BookgDt"), fmt);
    let value_date = date_choice(ntry.child("ValDt"), fmt);
    let bank_reference = ntry.find_text(&["AcctSvcrRef"]);
    let entry_reference = ntry.find_text(&["NtryRef"]);
    let bank_transaction_code = bank_tx_code(ntry.child("BkTxCd"));
    let entry_charges = charges_of(ntry);

    let details: Vec<&Node> = ntry
        .children_named("NtryDtls")
        .flat_map(|d| d.children_named("TxDtls"))
        .collect();

    let base = |amount: f64, cd: &str| Transaction {
        booking_date: booking_date.clone(),
        value_date: value_date.clone(),
        credit_debit: cd.to_string(),
        reversal,
        status: status.clone(),
        amount,
        currency: entry_currency.clone(),
        bank_transaction_code: bank_transaction_code.clone(),
        entry_reference: entry_reference.clone(),
        bank_reference: bank_reference.clone(),
        end_to_end_id: String::new(),
        counterparty: String::new(),
        counterparty_iban: String::new(),
        remittance_info: String::new(),
        charges: entry_charges,
        exchange_rate: None,
        details_count: None,
    };

    if details.is_empty() {
        out.push(base(apply_sign(entry_amount, &entry_cd, signed), &entry_cd));
        return Ok(());
    }

    if expand_details {
        for tx in &details {
            // A detail may carry its own amount/direction (batch entries);
            // otherwise it inherits the entry's.
            let cd = {
                let d = tx.find_text(&["CdtDbtInd"]);
                if d.is_empty() { entry_cd.clone() } else { d }
            };
            let (amount, currency) = detail_amount(tx, entry_amount, &entry_currency)?;
            let mut t = base(apply_sign(amount, &cd, signed), &cd);
            t.currency = currency;
            fill_from_detail(&mut t, tx, &cd);
            out.push(t);
        }
    } else {
        let mut t = base(apply_sign(entry_amount, &entry_cd, signed), &entry_cd);
        // Roll-up row: entry-level amount, references from the first detail.
        fill_from_detail(&mut t, details[0], &entry_cd);
        if details.len() > 1 {
            t.details_count = Some(details.len());
        }
        out.push(t);
    }
    Ok(())
}

/// A `TxDtls` amount: its own `Amt`, else `AmtDtls/TxAmt/Amt`, else the entry's.
fn detail_amount(
    tx: &Node,
    entry_amount: f64,
    entry_currency: &str,
) -> Result<(f64, String), String> {
    let node = tx
        .child("Amt")
        .or_else(|| tx.find(&["AmtDtls", "TxAmt", "Amt"]));
    match node {
        Some(a) => {
            let amount = parse_amount(&a.text())
                .ok_or_else(|| format!("malformed <TxDtls> amount '{}'", a.text()))?;
            let ccy = a.attr("Ccy").unwrap_or(entry_currency).to_string();
            Ok((amount, ccy))
        }
        None => Ok((entry_amount, entry_currency.to_string())),
    }
}

/// Copy references, counterparty, remittance info, charges and FX rate from a
/// `TxDtls` into the row. `cd` is the row's effective direction: for DBIT
/// (money out) the counterparty is the creditor, for CRDT the debtor.
fn fill_from_detail(t: &mut Transaction, tx: &Node, cd: &str) {
    t.end_to_end_id = tx.find_text(&["Refs", "EndToEndId"]);
    if t.bank_reference.is_empty() {
        t.bank_reference = tx.find_text(&["Refs", "AcctSvcrRef"]);
    }

    if let Some(parties) = tx.child("RltdPties") {
        let (party, account) = if cd == "DBIT" {
            ("Cdtr", "CdtrAcct")
        } else {
            ("Dbtr", "DbtrAcct")
        };
        if let Some(p) = parties.child(party) {
            // v2: <Cdtr><Nm>…</Nm></Cdtr>; v8+: <Cdtr><Pty><Nm>…</Nm></Pty></Cdtr>.
            let nm = p.find_text(&["Nm"]);
            t.counterparty = if nm.is_empty() { p.find_text(&["Pty", "Nm"]) } else { nm };
        }
        t.counterparty_iban = parties.find_text(&[account, "Id", "IBAN"]);
    }

    if let Some(rmt) = tx.child("RmtInf") {
        let ustrd: Vec<String> = rmt
            .children_named("Ustrd")
            .map(|u| u.text())
            .filter(|s| !s.is_empty())
            .collect();
        t.remittance_info = if ustrd.is_empty() {
            rmt.find_text(&["Strd", "CdtrRefInf", "Ref"])
        } else {
            ustrd.join(" ")
        };
    }

    if let Some(c) = charges_of(tx) {
        t.charges = Some(c);
    }
    if let Some(r) =
        parse_amount(&tx.find_text(&["AmtDtls", "TxAmt", "CcyXchg", "XchgRate"]))
    {
        t.exchange_rate = Some(r);
    }
}

/// `Chrgs/TtlChrgsAndTaxAmt` on an entry or a `TxDtls`, when present.
fn charges_of(node: &Node) -> Option<f64> {
    parse_amount(&node.find_text(&["Chrgs", "TtlChrgsAndTaxAmt"]))
}

/// `BkTxCd` → dotted `DOMN.FMLY.SUBFMLY`, or the proprietary code.
fn bank_tx_code(node: Option<&Node>) -> String {
    let Some(bk) = node else { return String::new() };
    if let Some(domn) = bk.child("Domn") {
        let parts: Vec<String> = [
            domn.find_text(&["Cd"]),
            domn.find_text(&["Fmly", "Cd"]),
            domn.find_text(&["Fmly", "SubFmlyCd"]),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
        if !parts.is_empty() {
            return parts.join(".");
        }
    }
    bk.find_text(&["Prtry", "Cd"])
}

/// A date-or-datetime choice node (`<X><Dt>…</Dt></X>` / `<X><DtTm>…</DtTm></X>`).
fn date_choice(node: Option<&Node>, fmt: DateFormat) -> String {
    let Some(n) = node else { return String::new() };
    let raw = {
        let d = n.find_text(&["Dt"]);
        if d.is_empty() { n.find_text(&["DtTm"]) } else { d }
    };
    format_date(&raw, fmt)
}

/// Parse an ISO decimal amount (dot decimal separator, e.g. `1234.56`).
fn parse_amount(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|f| f.is_finite())
}

/// Apply the credit/debit sign when `signed` is enabled. `DBIT` → negative.
fn apply_sign(amount: f64, credit_debit: &str, signed: bool) -> f64 {
    if signed && credit_debit == "DBIT" {
        -amount
    } else {
        amount
    }
}

/// Format an ISO date (`YYYY-MM-DD`, optionally with a `T…` time suffix) per
/// `fmt`. `raw` keeps the source string verbatim (including any time part);
/// non-ISO strings pass through unchanged.
fn format_date(raw: &str, fmt: DateFormat) -> String {
    if raw.is_empty() || fmt == DateFormat::Raw {
        return raw.to_string();
    }
    // First 10 chars = the date part (guarded: non-ASCII input falls through).
    let date = raw.get(..10).unwrap_or(raw);
    let b = date.as_bytes();
    let is_iso = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() }
        });
    if !is_iso {
        return raw.to_string();
    }
    let (yyyy, mm, dd) = (&date[0..4], &date[5..7], &date[8..10]);
    match fmt {
        DateFormat::Iso => date.to_string(),
        DateFormat::Us => format!("{mm}/{dd}/{yyyy}"),
        DateFormat::Eu => format!("{dd}/{mm}/{yyyy}"),
        DateFormat::Raw => raw.to_string(),
    }
}

// ---------------------------------------------------------------------------
// CSV rendering
// ---------------------------------------------------------------------------

fn delimiter_byte(delimiter: &str) -> Result<u8, String> {
    match delimiter.trim().to_ascii_lowercase().as_str() {
        "" | "comma" => Ok(b','),
        "semicolon" => Ok(b';'),
        "tab" => Ok(b'\t'),
        "pipe" => Ok(b'|'),
        other => Err(format!(
            "unknown delimiter '{other}' (use comma, semicolon, tab, or pipe)"
        )),
    }
}

/// Render every transaction across all statements as a flat CSV. A `Statement`
/// column disambiguates multi-statement files; balances live in the JSON output.
fn write_csv(statements: &[Statement], delimiter: &str) -> Result<String, String> {
    let delim = delimiter_byte(delimiter)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);
    wtr.write_record([
        "Statement",
        "Booking Date",
        "Value Date",
        "D/C",
        "Amount",
        "Currency",
        "Status",
        "Bank Transaction Code",
        "End To End Id",
        "Bank Reference",
        "Counterparty",
        "Counterparty IBAN",
        "Description",
    ])
    .map_err(|e| e.to_string())?;
    for (si, st) in statements.iter().enumerate() {
        for t in &st.transactions {
            let desc = t
                .remittance_info
                .split('\n')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ");
            wtr.write_record([
                &(si + 1).to_string(),
                &t.booking_date,
                &t.value_date,
                &t.credit_debit,
                &format!("{:.2}", t.amount),
                &t.currency,
                &t.status,
                &t.bank_transaction_code,
                &t.end_to_end_id,
                &t.bank_reference,
                &t.counterparty,
                &t.counterparty_iban,
                &desc,
            ])
            .map_err(|e| e.to_string())?;
        }
    }
    let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camt.053.001.02 statement: one DBIT payment, one CRDT salary.
    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02">
 <BkToCstmrStmt>
  <GrpHdr><MsgId>MSG001</MsgId><CreDtTm>2024-02-01T06:00:00</CreDtTm></GrpHdr>
  <Stmt>
   <Id>STMT-2024-001</Id>
   <ElctrncSeqNb>1</ElctrncSeqNb>
   <CreDtTm>2024-02-01T06:00:00</CreDtTm>
   <Acct><Id><IBAN>NL91ABNA0417164300</IBAN></Id><Ccy>EUR</Ccy><Ownr><Nm>Acme BV</Nm></Ownr></Acct>
   <Bal>
    <Tp><CdOrPrtry><Cd>OPBD</Cd></CdOrPrtry></Tp>
    <Amt Ccy="EUR">1000.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Dt><Dt>2024-01-01</Dt></Dt>
   </Bal>
   <Bal>
    <Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp>
    <Amt Ccy="EUR">2849.50</Amt><CdtDbtInd>CRDT</CdtDbtInd><Dt><Dt>2024-01-31</Dt></Dt>
   </Bal>
   <Ntry>
    <Amt Ccy="EUR">150.50</Amt>
    <CdtDbtInd>DBIT</CdtDbtInd>
    <Sts>BOOK</Sts>
    <BookgDt><Dt>2024-01-02</Dt></BookgDt>
    <ValDt><Dt>2024-01-02</Dt></ValDt>
    <AcctSvcrRef>BANKREF1</AcctSvcrRef>
    <BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>ICDT</Cd><SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn></BkTxCd>
    <NtryDtls><TxDtls>
     <Refs><EndToEndId>E2E-42</EndToEndId></Refs>
     <RltdPties><Cdtr><Nm>Acme Corp</Nm></Cdtr><CdtrAcct><Id><IBAN>DE89370400440532013000</IBAN></Id></CdtrAcct></RltdPties>
     <RmtInf><Ustrd>Payment invoice 42</Ustrd></RmtInf>
    </TxDtls></NtryDtls>
   </Ntry>
   <Ntry>
    <Amt Ccy="EUR">2000.00</Amt>
    <CdtDbtInd>CRDT</CdtDbtInd>
    <Sts>BOOK</Sts>
    <BookgDt><Dt>2024-01-03</Dt></BookgDt>
    <ValDt><Dt>2024-01-03</Dt></ValDt>
    <AcctSvcrRef>BANKREF2</AcctSvcrRef>
    <NtryDtls><TxDtls>
     <Refs><EndToEndId>PAYROLL-MAR</EndToEndId></Refs>
     <RltdPties><Dbtr><Nm>Payroll Ltd</Nm></Dbtr><DbtrAcct><Id><IBAN>GB29NWBK60161331926819</IBAN></Id></DbtrAcct></RltdPties>
     <RmtInf><Ustrd>Salary March</Ustrd></RmtInf>
    </TxDtls></NtryDtls>
   </Ntry>
  </Stmt>
 </BkToCstmrStmt>
</Document>"#;

    /// A batch entry: one Ntry rolling up two TxDtls payments.
    const BATCH: &str = r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.08">
 <BkToCstmrStmt>
  <Stmt>
   <Id>B1</Id>
   <Acct><Id><IBAN>NL91ABNA0417164300</IBAN></Id><Ccy>EUR</Ccy></Acct>
   <Ntry>
    <Amt Ccy="EUR">300.00</Amt>
    <CdtDbtInd>DBIT</CdtDbtInd>
    <Sts><Cd>BOOK</Cd></Sts>
    <BookgDt><Dt>2024-03-01</Dt></BookgDt>
    <ValDt><Dt>2024-03-01</Dt></ValDt>
    <NtryDtls>
     <TxDtls>
      <Refs><EndToEndId>E2E-A</EndToEndId></Refs>
      <Amt Ccy="EUR">100.00</Amt>
      <CdtDbtInd>DBIT</CdtDbtInd>
      <RltdPties><Cdtr><Pty><Nm>Alpha GmbH</Nm></Pty></Cdtr></RltdPties>
      <RmtInf><Ustrd>Rent</Ustrd></RmtInf>
     </TxDtls>
     <TxDtls>
      <Refs><EndToEndId>E2E-B</EndToEndId></Refs>
      <Amt Ccy="EUR">200.00</Amt>
      <CdtDbtInd>DBIT</CdtDbtInd>
      <RltdPties><Cdtr><Pty><Nm>Beta SARL</Nm></Pty></Cdtr></RltdPties>
      <RmtInf><Ustrd>Utilities</Ustrd></RmtInf>
     </TxDtls>
    </NtryDtls>
   </Ntry>
  </Stmt>
 </BkToCstmrStmt>
</Document>"#;

    #[test]
    fn parses_json_balances_and_transactions() {
        let out = parse(SAMPLE, Output::Json, DateFormat::Iso, "comma", true, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let st = &v[0];
        assert_eq!(st["message_type"], "camt.053");
        assert_eq!(st["statement_id"], "STMT-2024-001");
        assert_eq!(st["sequence_number"], "1");
        assert_eq!(st["creation_date"], "2024-02-01");
        assert_eq!(st["account_iban"], "NL91ABNA0417164300");
        assert_eq!(st["account_currency"], "EUR");
        assert_eq!(st["account_owner"], "Acme BV");
        assert_eq!(st["opening_balance"]["amount"], 1000.0);
        assert_eq!(st["opening_balance"]["type_code"], "OPBD");
        assert_eq!(st["opening_balance"]["description"], "Opening booked");
        assert_eq!(st["opening_balance"]["date"], "2024-01-01");
        assert_eq!(st["closing_balance"]["amount"], 2849.5);
        assert_eq!(st["balances"].as_array().unwrap().len(), 2);
        assert_eq!(st["transaction_count"], 2);
        let t0 = &st["transactions"][0];
        assert_eq!(t0["booking_date"], "2024-01-02");
        assert_eq!(t0["value_date"], "2024-01-02");
        assert_eq!(t0["credit_debit"], "DBIT");
        assert_eq!(t0["status"], "BOOK");
        assert_eq!(t0["amount"], -150.5); // DBIT → negative when signed
        assert_eq!(t0["currency"], "EUR");
        assert_eq!(t0["bank_transaction_code"], "PMNT.ICDT.ESCT");
        assert_eq!(t0["bank_reference"], "BANKREF1");
        assert_eq!(t0["end_to_end_id"], "E2E-42");
        assert_eq!(t0["counterparty"], "Acme Corp"); // DBIT → creditor
        assert_eq!(t0["counterparty_iban"], "DE89370400440532013000");
        assert_eq!(t0["remittance_info"], "Payment invoice 42");
        let t1 = &st["transactions"][1];
        assert_eq!(t1["amount"], 2000.0); // CRDT → positive
        assert_eq!(t1["counterparty"], "Payroll Ltd"); // CRDT → debtor
    }

    #[test]
    fn csv_output_has_header_and_rows() {
        let out = parse(SAMPLE, Output::Csv, DateFormat::Iso, "comma", true, true).unwrap();
        let mut lines = out.lines();
        assert_eq!(
            lines.next().unwrap(),
            "Statement,Booking Date,Value Date,D/C,Amount,Currency,Status,Bank Transaction Code,End To End Id,Bank Reference,Counterparty,Counterparty IBAN,Description"
        );
        assert_eq!(
            lines.next().unwrap(),
            "1,2024-01-02,2024-01-02,DBIT,-150.50,EUR,BOOK,PMNT.ICDT.ESCT,E2E-42,BANKREF1,Acme Corp,DE89370400440532013000,Payment invoice 42"
        );
        assert_eq!(
            lines.next().unwrap(),
            "1,2024-01-03,2024-01-03,CRDT,2000.00,EUR,BOOK,,PAYROLL-MAR,BANKREF2,Payroll Ltd,GB29NWBK60161331926819,Salary March"
        );
    }

    #[test]
    fn batch_entry_expands_to_one_row_per_detail() {
        let out = parse(BATCH, Output::Csv, DateFormat::Iso, "comma", true, true).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 payments
        assert!(lines[1].contains("-100.00"));
        assert!(lines[1].contains("Alpha GmbH")); // v8+ Pty/Nm nesting
        assert!(lines[1].contains("Rent"));
        assert!(lines[2].contains("-200.00"));
        assert!(lines[2].contains("Beta SARL"));
    }

    #[test]
    fn batch_entry_rolls_up_when_not_expanding() {
        let out = parse(BATCH, Output::Json, DateFormat::Iso, "comma", true, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let txs = v[0]["transactions"].as_array().unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0]["amount"], -300.0); // entry-level total
        assert_eq!(txs[0]["details_count"], 2);
        assert_eq!(txs[0]["status"], "BOOK"); // v8+ Sts/Cd nesting
        assert_eq!(txs[0]["end_to_end_id"], "E2E-A"); // first detail's refs
    }

    #[test]
    fn unsigned_amounts_stay_positive() {
        let out = parse(SAMPLE, Output::Json, DateFormat::Iso, "comma", false, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["transactions"][0]["amount"], 150.5);
        assert_eq!(v[0]["transactions"][0]["credit_debit"], "DBIT");
    }

    #[test]
    fn date_formats() {
        let us = parse(SAMPLE, Output::Json, DateFormat::Us, "comma", true, true).unwrap();
        assert!(us.contains("01/02/2024"));
        let eu = parse(SAMPLE, Output::Json, DateFormat::Eu, "comma", true, true).unwrap();
        assert!(eu.contains("02/01/2024"));
        let raw = parse(SAMPLE, Output::Json, DateFormat::Raw, "comma", true, true).unwrap();
        assert!(raw.contains("2024-02-01T06:00:00")); // datetime kept verbatim
    }

    #[test]
    fn camt052_report_is_accepted() {
        let data = r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.02">
 <BkToCstmrAcctRpt>
  <Rpt>
   <Id>R1</Id>
   <Acct><Id><IBAN>FR1420041010050500013M02606</IBAN></Id></Acct>
   <Ntry>
    <Amt Ccy="EUR">10.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts>BOOK</Sts>
    <BookgDt><Dt>2024-05-01</Dt></BookgDt><ValDt><Dt>2024-05-02</Dt></ValDt>
   </Ntry>
  </Rpt>
 </BkToCstmrAcctRpt>
</Document>"#;
        let out = parse(data, Output::Json, DateFormat::Iso, "comma", true, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["message_type"], "camt.052");
        assert_eq!(v[0]["transactions"][0]["amount"], 10.0);
        assert_eq!(v[0]["transactions"][0]["value_date"], "2024-05-02");
    }

    #[test]
    fn reversal_and_pending_flags() {
        let data = r#"<Document><BkToCstmrStmt><Stmt><Id>X</Id>
 <Ntry><Amt Ccy="EUR">5.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><RvslInd>true</RvslInd>
 <Sts>PDNG</Sts><BookgDt><Dt>2024-06-01</Dt></BookgDt></Ntry>
</Stmt></BkToCstmrStmt></Document>"#;
        let out = parse(data, Output::Json, DateFormat::Iso, "comma", true, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["transactions"][0]["reversal"], true);
        assert_eq!(v[0]["transactions"][0]["status"], "PDNG");
    }

    #[test]
    fn semicolon_delimiter() {
        let out = parse(SAMPLE, Output::Csv, DateFormat::Iso, "semicolon", true, true).unwrap();
        assert!(out.starts_with("Statement;Booking Date;"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse("   ", Output::Json, DateFormat::Iso, "comma", true, true).is_err());
    }

    #[test]
    fn non_camt_xml_errors() {
        let err = parse(
            "<note><to>You</to></note>",
            Output::Json,
            DateFormat::Iso,
            "comma",
            true,
            true,
        )
        .unwrap_err();
        assert!(err.contains("BkToCstmrStmt"));
    }

    #[test]
    fn malformed_xml_errors() {
        let err = parse(
            "<Document><BkToCstmrStmt>",
            Output::Json,
            DateFormat::Iso,
            "comma",
            true,
            true,
        )
        .unwrap_err();
        assert!(err.contains("malformed XML"));
    }

    #[test]
    fn empty_message_errors() {
        let err = parse(
            "<Document><BkToCstmrStmt><GrpHdr><MsgId>M</MsgId></GrpHdr></BkToCstmrStmt></Document>",
            Output::Json,
            DateFormat::Iso,
            "comma",
            true,
            true,
        )
        .unwrap_err();
        assert!(err.contains("<Stmt>"));
    }

    #[test]
    fn bad_delimiter_errors() {
        assert!(parse(SAMPLE, Output::Csv, DateFormat::Iso, "colon", true, true).is_err());
    }

    #[test]
    fn enum_parsers() {
        assert!(Output::parse("csv").is_ok());
        assert!(Output::parse("xml").is_err());
        assert!(DateFormat::parse("julian").is_err());
    }

    #[test]
    fn charges_and_exchange_rate() {
        let data = r#"<Document><BkToCstmrStmt><Stmt><Id>C</Id>
 <Ntry><Amt Ccy="EUR">92.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts>
  <BookgDt><Dt>2024-07-01</Dt></BookgDt>
  <NtryDtls><TxDtls>
   <AmtDtls><TxAmt><Amt Ccy="USD">100.00</Amt><CcyXchg><XchgRate>0.92</XchgRate></CcyXchg></TxAmt></AmtDtls>
   <Chrgs><TtlChrgsAndTaxAmt>1.50</TtlChrgsAndTaxAmt></Chrgs>
  </TxDtls></NtryDtls>
 </Ntry>
</Stmt></BkToCstmrStmt></Document>"#;
        let out = parse(data, Output::Json, DateFormat::Iso, "comma", true, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let t = &v[0]["transactions"][0];
        assert_eq!(t["amount"], -100.0); // AmtDtls/TxAmt amount, in USD
        assert_eq!(t["currency"], "USD");
        assert_eq!(t["exchange_rate"], 0.92);
        assert_eq!(t["charges"], 1.5);
    }

    #[test]
    fn run_string_entry_point_parses_json() {
        let out = run(SAMPLE, "json", "iso", "comma", true, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["transactions"][0]["amount"], -150.5);
    }

    #[test]
    fn run_rejects_unknown_output_selector() {
        let err = run(SAMPLE, "yaml", "iso", "comma", true, true).unwrap_err();
        assert!(err.contains("yaml"));
    }
}
