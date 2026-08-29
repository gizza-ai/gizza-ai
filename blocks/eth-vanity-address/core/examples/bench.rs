use std::time::Instant;
fn main() {
    let start = gizza_ai_eth_vanity_address_core::start_key_from_seed("bench");
    let t = Instant::now();
    let n = 50_000u64;
    let r = gizza_ai_eth_vanity_address_core::search("ffffffffff", "", false, n, &start).unwrap();
    let el = t.elapsed();
    println!("{n} attempts in {:?} => {:.0} keys/s (found={:?})", el, n as f64 / el.as_secs_f64(), r.is_some());
}
