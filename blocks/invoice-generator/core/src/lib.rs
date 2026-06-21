//! gizza-ai/invoice-generator core — turn line items into a formatted, printable
//! PDF invoice. Pure-Rust (`lopdf`, base-14 Helvetica — no font files). One Letter
//! page; text laid out with absolute coordinates.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
}

/// Parse pipe-delimited line items, one per line: `Description | qty | unit_price`.
pub fn parse_items(s: &str) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    for (i, line) in s.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let parts: Vec<&str> = l.split('|').map(|p| p.trim()).collect();
        if parts.len() != 3 {
            return Err(format!(
                "line {} must be 'description | quantity | unit_price': {l}",
                i + 1
            ));
        }
        let num = |x: &str| -> Result<f64, String> {
            x.replace([',', '$', '£', '€'], "")
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("line {}: '{x}' is not a number", i + 1))
        };
        items.push(Item {
            description: parts[0].to_string(),
            quantity: num(parts[1])?,
            unit_price: num(parts[2])?,
        });
    }
    if items.is_empty() {
        return Err("no line items provided".into());
    }
    Ok(items)
}

/// One text element at absolute (x, y) in PDF points (origin bottom-left).
fn text_ops(ops: &mut Vec<Operation>, font: &str, size: f64, x: f64, y: f64, s: &str) {
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new("Tf", vec![font.into(), size.into()]));
    ops.push(Operation::new("Td", vec![x.into(), y.into()]));
    ops.push(Operation::new("Tj", vec![Object::string_literal(s)]));
    ops.push(Operation::new("ET", vec![]));
}

const MAX_ROWS: usize = 28;

/// Build a one-page PDF invoice. `currency` is a short prefix like "$".
pub fn generate(
    seller: &str,
    client: &str,
    invoice_number: &str,
    date: &str,
    items: &[Item],
    tax_rate: f64,
    currency: &str,
    notes: &str,
) -> Result<Vec<u8>, String> {
    if items.is_empty() {
        return Err("no line items".into());
    }
    let cur = if currency.is_empty() { "$" } else { currency };
    let money = |v: f64| format!("{cur}{v:.2}");

    let subtotal: f64 = items.iter().map(|it| it.quantity * it.unit_price).sum();
    let tax = subtotal * tax_rate / 100.0;
    let total = subtotal + tax;

    let mut ops: Vec<Operation> = Vec::new();

    // Title.
    text_ops(&mut ops, "F2", 26.0, 50.0, 740.0, "INVOICE");
    // Invoice meta (right side).
    if !invoice_number.is_empty() {
        text_ops(&mut ops, "F1", 11.0, 400.0, 744.0, &format!("Invoice #: {invoice_number}"));
    }
    if !date.is_empty() {
        text_ops(&mut ops, "F1", 11.0, 400.0, 728.0, &format!("Date: {date}"));
    }

    // From / Bill To.
    text_ops(&mut ops, "F2", 11.0, 50.0, 700.0, "From:");
    let mut y = 686.0;
    for line in seller.lines().take(6) {
        text_ops(&mut ops, "F1", 10.0, 50.0, y, line);
        y -= 13.0;
    }
    text_ops(&mut ops, "F2", 11.0, 320.0, 700.0, "Bill To:");
    let mut y2 = 686.0;
    for line in client.lines().take(6) {
        text_ops(&mut ops, "F1", 10.0, 320.0, y2, line);
        y2 -= 13.0;
    }

    // Table header.
    let header_y = 600.0;
    text_ops(&mut ops, "F2", 10.0, 50.0, header_y, "Description");
    text_ops(&mut ops, "F2", 10.0, 360.0, header_y, "Qty");
    text_ops(&mut ops, "F2", 10.0, 420.0, header_y, "Unit");
    text_ops(&mut ops, "F2", 10.0, 500.0, header_y, "Amount");
    // header rule
    ops.push(Operation::new("m", vec![50.0.into(), (header_y - 4.0).into()]));
    ops.push(Operation::new("l", vec![562.0.into(), (header_y - 4.0).into()]));
    ops.push(Operation::new("S", vec![]));

    // Rows.
    let mut ry = header_y - 20.0;
    let shown = items.len().min(MAX_ROWS);
    for it in items.iter().take(MAX_ROWS) {
        let amount = it.quantity * it.unit_price;
        let desc: String = it.description.chars().take(48).collect();
        text_ops(&mut ops, "F1", 10.0, 50.0, ry, &desc);
        text_ops(&mut ops, "F1", 10.0, 360.0, ry, &format!("{}", trim_num(it.quantity)));
        text_ops(&mut ops, "F1", 10.0, 420.0, ry, &money(it.unit_price));
        text_ops(&mut ops, "F1", 10.0, 500.0, ry, &money(amount));
        ry -= 16.0;
    }
    if items.len() > MAX_ROWS {
        text_ops(&mut ops, "F1", 9.0, 50.0, ry, &format!("… and {} more item(s) not shown", items.len() - MAX_ROWS));
        ry -= 16.0;
    }

    // Totals.
    let mut ty = ry - 10.0;
    ops.push(Operation::new("m", vec![380.0.into(), (ty + 12.0).into()]));
    ops.push(Operation::new("l", vec![562.0.into(), (ty + 12.0).into()]));
    ops.push(Operation::new("S", vec![]));
    text_ops(&mut ops, "F1", 10.0, 400.0, ty, "Subtotal:");
    text_ops(&mut ops, "F1", 10.0, 500.0, ty, &money(subtotal));
    ty -= 16.0;
    text_ops(&mut ops, "F1", 10.0, 400.0, ty, &format!("Tax ({}%):", trim_num(tax_rate)));
    text_ops(&mut ops, "F1", 10.0, 500.0, ty, &money(tax));
    ty -= 18.0;
    text_ops(&mut ops, "F2", 12.0, 400.0, ty, "Total:");
    text_ops(&mut ops, "F2", 12.0, 500.0, ty, &money(total));

    // Notes.
    if !notes.trim().is_empty() {
        let mut ny = (ty - 40.0).max(60.0);
        text_ops(&mut ops, "F2", 10.0, 50.0, ny + 14.0, "Notes:");
        for line in notes.lines().take(6) {
            text_ops(&mut ops, "F1", 9.0, 50.0, ny, &line.chars().take(110).collect::<String>());
            ny -= 12.0;
        }
    }
    let _ = shown;

    // Assemble the document.
    let mut doc = Document::with_version("1.5");
    let font_reg = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let font_bold = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold",
    });
    let resources = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_reg, "F2" => font_bold },
    });
    let content = Content { operations: ops };
    let content_data = content.encode().map_err(|e| format!("encode content: {e}"))?;
    let content_id = doc.add_object(Stream::new(dictionary! {}, content_data));
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
    });
    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).map_err(|e| format!("save PDF: {e}"))?;
    Ok(buf)
}

/// Drop a trailing ".0" so 3.0 prints as "3" but 2.5 stays "2.5".
fn trim_num(v: f64) -> String {
    if (v.fract()).abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_items() {
        let items = parse_items("Widget | 3 | 9.99\nGadget | 1 | 100").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].description, "Widget");
        assert_eq!(items[0].quantity, 3.0);
        assert_eq!(items[1].unit_price, 100.0);
    }

    #[test]
    fn strips_currency_in_items() {
        let items = parse_items("Thing | 2 | $1,250.50").unwrap();
        assert_eq!(items[0].unit_price, 1250.50);
    }

    #[test]
    fn item_parse_errors() {
        assert!(parse_items("bad line").is_err());
        assert!(parse_items("x | two | 5").is_err());
        assert!(parse_items("").is_err());
    }

    #[test]
    fn generates_pdf() {
        let items = parse_items("Design work | 10 | 75\nHosting | 1 | 20").unwrap();
        let pdf = generate(
            "ACME LLC\n1 Main St",
            "Client Co\n2 Side Rd",
            "INV-001",
            "2024-01-15",
            &items,
            8.5,
            "$",
            "Thanks for your business.",
        )
        .unwrap();
        assert_eq!(&pdf[0..5], b"%PDF-");
        assert!(pdf.windows(5).any(|w| w == b"%%EOF") || pdf.len() > 500);
        // Re-parse with lopdf to confirm it's a valid, loadable PDF.
        let doc = Document::load_mem(&pdf).unwrap();
        assert!(doc.get_pages().len() == 1);
    }

    #[test]
    fn totals_math_via_pdf_is_nonempty() {
        let items = parse_items("A | 2 | 10").unwrap();
        let pdf = generate("S", "C", "1", "today", &items, 10.0, "$", "").unwrap();
        assert!(pdf.len() > 400);
    }
}
