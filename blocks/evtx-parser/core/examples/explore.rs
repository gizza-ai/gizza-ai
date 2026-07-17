use evtx::{EvtxParser, ParserSettings};
fn main() {
    let bytes = include_bytes!("../tests/fixtures/security-sample.evtx").to_vec();
    let settings = ParserSettings::new().separate_json_attributes(true);
    let mut parser = EvtxParser::from_buffer(bytes).unwrap().with_configuration(settings);
    let mut n = 0;
    for r in parser.records_json_value() {
        let r = r.unwrap();
        if n < 2 {
            println!("=== record {} id={} ts={} ===", n, r.event_record_id, r.timestamp);
            println!("{}", serde_json::to_string_pretty(&r.data).unwrap());
        }
        n += 1;
    }
    println!("TOTAL records: {}", n);
}
