//! fit-file-decoder core — pure compute, shared by the chat skill block and the web page.
//!
//! Decodes a binary Garmin/ANT FIT activity file (given as base64 bytes) into readable
//! records, then exports them as a human summary, a CSV table, or a GPX track. No
//! wafer/wasm-bindgen deps and no external crates — a small self-contained FIT reader plus a
//! hand-rolled base64 decoder, so it instantiates on every gizza surface (chat / CLI / page).
//!
//! Scope (bounded, activity-oriented): FIT header, definition + data records, both little- and
//! big-endian architectures, developer fields (skipped safely), and the primitive record-message
//! fields an activity needs — timestamp, position (semicircles → degrees), altitude, distance,
//! speed, heart rate, cadence and power — plus the session summary (sport + totals/averages) when
//! present. It is not a full FIT profile decoder; unknown messages/fields are skipped, not errored.

/// Max decoded FIT payload accepted (keeps the wasm sandbox well inside its memory budget).
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Seconds between the Unix epoch and the FIT epoch (1989-12-31T00:00:00Z).
const FIT_EPOCH: i64 = 631_065_600;

/// Semicircles per degree = 2^31 / 180.
const SEMICIRCLES_PER_DEG: f64 = 2_147_483_648.0 / 180.0;

/// Output shape.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Summary,
    Csv,
    Gpx,
}

impl Format {
    fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "summary" => Ok(Format::Summary),
            "csv" => Ok(Format::Csv),
            "gpx" => Ok(Format::Gpx),
            other => Err(format!(
                "unknown format '{other}' — expected one of: summary, csv, gpx"
            )),
        }
    }
}

/// One decoded record (`record` message, global msg 20). Every field is optional because a FIT
/// record only carries whatever the device sampled at that instant.
#[derive(Default, Clone)]
pub struct RecordPoint {
    pub unix_time: Option<i64>,
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    pub altitude_m: Option<f64>,
    pub distance_m: Option<f64>,
    pub speed_mps: Option<f64>,
    pub heart_rate: Option<u32>,
    pub cadence: Option<u32>,
    pub power: Option<u32>,
}

impl RecordPoint {
    fn is_empty(&self) -> bool {
        self.unix_time.is_none()
            && self.lat_deg.is_none()
            && self.lon_deg.is_none()
            && self.altitude_m.is_none()
            && self.distance_m.is_none()
            && self.speed_mps.is_none()
            && self.heart_rate.is_none()
            && self.cadence.is_none()
            && self.power.is_none()
    }
}

/// Session summary (`session` message, global msg 18) — only the fields we surface.
#[derive(Default, Clone)]
pub struct Session {
    pub sport: Option<u32>,
    pub start_time: Option<i64>,
    pub total_elapsed_s: Option<f64>,
    pub total_timer_s: Option<f64>,
    pub total_distance_m: Option<f64>,
    pub total_calories: Option<u32>,
    pub avg_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    pub avg_hr: Option<u32>,
    pub max_hr: Option<u32>,
    pub avg_power: Option<u32>,
    pub max_power: Option<u32>,
    pub total_ascent_m: Option<u32>,
    pub total_descent_m: Option<u32>,
}

impl Session {
    fn has_any(&self) -> bool {
        self.sport.is_some()
            || self.start_time.is_some()
            || self.total_elapsed_s.is_some()
            || self.total_distance_m.is_some()
            || self.total_calories.is_some()
            || self.avg_speed_mps.is_some()
            || self.avg_hr.is_some()
            || self.avg_power.is_some()
    }
}

/// Everything decoded from a FIT file.
pub struct Decoded {
    pub protocol_major: u8,
    pub protocol_minor: u8,
    pub profile_version: u16,
    pub data_size: u32,
    pub records: Vec<RecordPoint>,
    pub session: Option<Session>,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Decode base64-encoded FIT bytes and render in the requested `format`.
pub fn decode_str(data_b64: &str, format: &str) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    let bytes = b64_decode(data_b64)?;
    if bytes.is_empty() {
        return Err("no input — paste the base64-encoded contents of a .fit file".into());
    }
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "FIT file is {} bytes; the maximum supported size is {} bytes (8 MiB). Split or trim the activity first.",
            bytes.len(),
            MAX_BYTES
        ));
    }
    let dec = parse_fit(&bytes)?;
    Ok(render(&dec, fmt))
}

/// Decode raw FIT bytes (used by the CLI/chat which may already hold the bytes) and render.
pub fn decode_bytes(bytes: &[u8], format: &str) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "FIT file is {} bytes; the maximum supported size is {} bytes (8 MiB).",
            bytes.len(),
            MAX_BYTES
        ));
    }
    let dec = parse_fit(bytes)?;
    Ok(render(&dec, fmt))
}

// ---------------------------------------------------------------------------
// FIT parser
// ---------------------------------------------------------------------------

struct FieldDef {
    num: u8,
    size: usize,
}

struct MsgDef {
    big_endian: bool,
    global_num: u16,
    fields: Vec<FieldDef>,
    dev_size: usize,
}

fn parse_fit(bytes: &[u8]) -> Result<Decoded, String> {
    if bytes.len() < 12 {
        return Err("file is too short to be a FIT file (needs at least a 12-byte header)".into());
    }
    let header_size = bytes[0] as usize;
    if header_size < 12 || header_size > bytes.len() {
        return Err(format!(
            "invalid FIT header size byte ({header_size}); not a FIT file"
        ));
    }
    // The ".FIT" signature lives at bytes 8..12 of the header.
    if &bytes[8..12] != b".FIT" {
        return Err("not a FIT file — missing the \".FIT\" signature in the header".into());
    }
    let protocol = bytes[1];
    let profile_version = u16::from_le_bytes([bytes[2], bytes[3]]);
    let data_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

    // Data records span [header_size, header_size + data_size); guard against a bogus data_size.
    let declared_end = header_size.saturating_add(data_size as usize);
    let data_end = if data_size == 0 || declared_end > bytes.len() {
        // Fall back to the whole file minus a trailing 2-byte CRC when present.
        bytes.len().saturating_sub(2).max(header_size)
    } else {
        declared_end
    };

    let mut defs: [Option<MsgDef>; 16] = Default::default();
    let mut records: Vec<RecordPoint> = Vec::new();
    let mut session: Option<Session> = None;
    let mut last_ts: Option<u32> = None; // FIT-epoch seconds, for compressed timestamps

    let mut pos = header_size;
    while pos < data_end {
        let rec_header = bytes[pos];
        pos += 1;

        if rec_header & 0x80 != 0 {
            // Compressed-timestamp data message: bits 5-6 = local type, bits 0-4 = time offset.
            let local = ((rec_header >> 5) & 0x03) as usize;
            let offset = (rec_header & 0x1F) as u32;
            let def = defs[local]
                .as_ref()
                .ok_or("corrupt FIT: data record references an undefined message")?;
            let full_ts = match last_ts {
                Some(prev) => {
                    let mut t = (prev & !0x1F) | offset;
                    if offset < (prev & 0x1F) {
                        t = t.wrapping_add(0x20);
                    }
                    t
                }
                None => offset,
            };
            last_ts = Some(full_ts);
            read_data_message(
                bytes,
                &mut pos,
                data_end,
                def,
                Some(full_ts),
                &mut records,
                &mut session,
            )?;
            continue;
        }

        let local = (rec_header & 0x0F) as usize;
        let is_def = rec_header & 0x40 != 0;
        let has_dev = rec_header & 0x20 != 0;

        if is_def {
            let def = read_definition(bytes, &mut pos, data_end, has_dev)?;
            defs[local] = Some(def);
        } else {
            let def = defs[local]
                .as_ref()
                .ok_or("corrupt FIT: data record references an undefined message")?;
            read_data_message(bytes, &mut pos, data_end, def, None, &mut records, &mut session)?;
            if let Some(last) = records.last() {
                if let Some(t) = last.unix_time {
                    last_ts = Some((t - FIT_EPOCH) as u32);
                }
            }
        }
    }

    Ok(Decoded {
        protocol_major: protocol >> 4,
        protocol_minor: protocol & 0x0F,
        profile_version,
        data_size,
        records,
        session,
    })
}

fn read_definition(
    bytes: &[u8],
    pos: &mut usize,
    end: usize,
    has_dev: bool,
) -> Result<MsgDef, String> {
    // reserved(1) arch(1) global(2) nfields(1)
    need(*pos, 5, end)?;
    let big_endian = bytes[*pos + 1] != 0;
    let global_num = if big_endian {
        u16::from_be_bytes([bytes[*pos + 2], bytes[*pos + 3]])
    } else {
        u16::from_le_bytes([bytes[*pos + 2], bytes[*pos + 3]])
    };
    let nfields = bytes[*pos + 4] as usize;
    *pos += 5;

    need(*pos, nfields * 3, end)?;
    let mut fields = Vec::with_capacity(nfields);
    for _ in 0..nfields {
        let num = bytes[*pos];
        let size = bytes[*pos + 1] as usize;
        // base type at bytes[*pos + 2] is not needed: we read by declared size + architecture.
        fields.push(FieldDef { num, size });
        *pos += 3;
    }

    let mut dev_size = 0usize;
    if has_dev {
        need(*pos, 1, end)?;
        let ndev = bytes[*pos] as usize;
        *pos += 1;
        need(*pos, ndev * 3, end)?;
        for _ in 0..ndev {
            dev_size += bytes[*pos + 1] as usize; // size byte of each developer field
            *pos += 3;
        }
    }

    Ok(MsgDef {
        big_endian,
        global_num,
        fields,
        dev_size,
    })
}

#[allow(clippy::too_many_arguments)]
fn read_data_message(
    bytes: &[u8],
    pos: &mut usize,
    end: usize,
    def: &MsgDef,
    compressed_ts: Option<u32>,
    records: &mut Vec<RecordPoint>,
    session: &mut Option<Session>,
) -> Result<(), String> {
    let is_record = def.global_num == 20;
    let is_session = def.global_num == 18;

    let mut rec = RecordPoint::default();
    if let Some(ts) = compressed_ts {
        rec.unix_time = Some(ts as i64 + FIT_EPOCH);
    }
    let mut sess = Session::default();

    for f in &def.fields {
        need(*pos, f.size, end)?;
        let raw = &bytes[*pos..*pos + f.size];
        if is_record {
            apply_record_field(&mut rec, f.num, raw, def.big_endian);
        } else if is_session {
            apply_session_field(&mut sess, f.num, raw, def.big_endian);
        }
        *pos += f.size;
    }
    // Skip developer-field bytes (their values are user-defined and out of scope).
    need(*pos, def.dev_size, end)?;
    *pos += def.dev_size;

    if is_record && !rec.is_empty() {
        records.push(rec);
    } else if is_session && sess.has_any() && session.is_none() {
        *session = Some(sess);
    }
    Ok(())
}

fn apply_record_field(rec: &mut RecordPoint, num: u8, raw: &[u8], be: bool) {
    match num {
        253 => {
            if let Some(v) = u_val(raw, be, 4) {
                rec.unix_time = Some(v as i64 + FIT_EPOCH);
            }
        }
        0 => {
            if let Some(v) = i32_val(raw, be) {
                rec.lat_deg = Some(v as f64 / SEMICIRCLES_PER_DEG);
            }
        }
        1 => {
            if let Some(v) = i32_val(raw, be) {
                rec.lon_deg = Some(v as f64 / SEMICIRCLES_PER_DEG);
            }
        }
        2 => {
            if rec.altitude_m.is_none() {
                if let Some(v) = u_val(raw, be, 2) {
                    rec.altitude_m = Some(v as f64 / 5.0 - 500.0);
                }
            }
        }
        78 => {
            // enhanced_altitude (uint32) — preferred if present.
            if let Some(v) = u_val(raw, be, 4) {
                rec.altitude_m = Some(v as f64 / 5.0 - 500.0);
            }
        }
        5 => {
            if let Some(v) = u_val(raw, be, 4) {
                rec.distance_m = Some(v as f64 / 100.0);
            }
        }
        6 => {
            if rec.speed_mps.is_none() {
                if let Some(v) = u_val(raw, be, 2) {
                    rec.speed_mps = Some(v as f64 / 1000.0);
                }
            }
        }
        73 => {
            // enhanced_speed (uint32) — preferred if present.
            if let Some(v) = u_val(raw, be, 4) {
                rec.speed_mps = Some(v as f64 / 1000.0);
            }
        }
        3 => {
            if let Some(v) = u_val(raw, be, 1) {
                rec.heart_rate = Some(v as u32);
            }
        }
        4 => {
            if let Some(v) = u_val(raw, be, 1) {
                rec.cadence = Some(v as u32);
            }
        }
        7 => {
            if let Some(v) = u_val(raw, be, 2) {
                rec.power = Some(v as u32);
            }
        }
        _ => {}
    }
}

fn apply_session_field(s: &mut Session, num: u8, raw: &[u8], be: bool) {
    match num {
        5 => s.sport = u_val(raw, be, 1).map(|v| v as u32),
        2 => s.start_time = u_val(raw, be, 4).map(|v| v as i64 + FIT_EPOCH),
        7 => s.total_elapsed_s = u_val(raw, be, 4).map(|v| v as f64 / 1000.0),
        8 => s.total_timer_s = u_val(raw, be, 4).map(|v| v as f64 / 1000.0),
        9 => s.total_distance_m = u_val(raw, be, 4).map(|v| v as f64 / 100.0),
        11 => s.total_calories = u_val(raw, be, 2).map(|v| v as u32),
        14 => s.avg_speed_mps = u_val(raw, be, 2).map(|v| v as f64 / 1000.0),
        15 => s.max_speed_mps = u_val(raw, be, 2).map(|v| v as f64 / 1000.0),
        16 => s.avg_hr = u_val(raw, be, 1).map(|v| v as u32),
        17 => s.max_hr = u_val(raw, be, 1).map(|v| v as u32),
        20 => s.avg_power = u_val(raw, be, 2).map(|v| v as u32),
        21 => s.max_power = u_val(raw, be, 2).map(|v| v as u32),
        22 => s.total_ascent_m = u_val(raw, be, 2).map(|v| v as u32),
        23 => s.total_descent_m = u_val(raw, be, 2).map(|v| v as u32),
        _ => {}
    }
}

/// Read an unsigned integer of exactly `want` bytes; return None on an all-0xFF invalid value or a
/// size mismatch (arrays / unexpected widths are left unextracted).
fn u_val(raw: &[u8], be: bool, want: usize) -> Option<u64> {
    if raw.len() != want || want == 0 || want > 8 {
        return None;
    }
    if raw.iter().all(|&b| b == 0xFF) {
        return None; // FIT "invalid" sentinel for unsigned base types
    }
    let mut v: u64 = 0;
    if be {
        for &b in raw {
            v = (v << 8) | b as u64;
        }
    } else {
        for &b in raw.iter().rev() {
            v = (v << 8) | b as u64;
        }
    }
    Some(v)
}

/// Read a signed 32-bit integer (position semicircles); FIT invalid is 0x7FFFFFFF.
fn i32_val(raw: &[u8], be: bool) -> Option<i32> {
    if raw.len() != 4 {
        return None;
    }
    let v = if be {
        i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]])
    } else {
        i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])
    };
    if v == 0x7FFF_FFFF {
        None
    } else {
        Some(v)
    }
}

fn need(pos: usize, want: usize, end: usize) -> Result<(), String> {
    if pos.saturating_add(want) > end {
        Err("corrupt or truncated FIT file (record extends past the data section)".into())
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(dec: &Decoded, fmt: Format) -> String {
    match fmt {
        Format::Summary => render_summary(dec),
        Format::Csv => render_csv(dec),
        Format::Gpx => render_gpx(dec),
    }
}

fn render_summary(dec: &Decoded) -> String {
    let mut o = String::new();
    o.push_str("FIT file summary\n");
    o.push_str("================\n");
    o.push_str(&format!(
        "Protocol version: {}.{}\n",
        dec.protocol_major, dec.protocol_minor
    ));
    o.push_str(&format!(
        "Profile version:  {}.{:02}\n",
        dec.profile_version / 100,
        dec.profile_version % 100
    ));
    o.push_str(&format!("Data section:     {} bytes\n", dec.data_size));
    o.push_str(&format!("Records decoded:  {}\n", dec.records.len()));

    // Time range + bounding box from records.
    let times: Vec<i64> = dec.records.iter().filter_map(|r| r.unix_time).collect();
    if let (Some(&first), Some(&last)) = (times.first(), times.last()) {
        let dur = (last - first).max(0);
        o.push_str(&format!(
            "Time range:       {} → {} ({})\n",
            fmt_iso(first),
            fmt_iso(last),
            fmt_dur(dur as f64)
        ));
    }
    let gps: Vec<(f64, f64)> = dec
        .records
        .iter()
        .filter_map(|r| Some((r.lat_deg?, r.lon_deg?)))
        .collect();
    if !gps.is_empty() {
        let mut lat_min = f64::INFINITY;
        let mut lat_max = f64::NEG_INFINITY;
        let mut lon_min = f64::INFINITY;
        let mut lon_max = f64::NEG_INFINITY;
        for (la, lo) in &gps {
            lat_min = lat_min.min(*la);
            lat_max = lat_max.max(*la);
            lon_min = lon_min.min(*lo);
            lon_max = lon_max.max(*lo);
        }
        o.push_str(&format!(
            "GPS points:       {} (lat {:.6}..{:.6}, lon {:.6}..{:.6})\n",
            gps.len(),
            lat_min,
            lat_max,
            lon_min,
            lon_max
        ));
    } else {
        o.push_str("GPS points:       0 (no position data)\n");
    }

    if let Some(s) = &dec.session {
        o.push('\n');
        o.push_str("Session\n");
        o.push_str("-------\n");
        if let Some(sp) = s.sport {
            o.push_str(&format!("Sport:            {}\n", sport_name(sp)));
        }
        if let Some(t) = s.start_time {
            o.push_str(&format!("Start:            {}\n", fmt_iso(t)));
        }
        if let Some(d) = s.total_distance_m {
            o.push_str(&format!("Total distance:   {:.2} km\n", d / 1000.0));
        }
        match (s.total_timer_s, s.total_elapsed_s) {
            (Some(t), Some(e)) => o.push_str(&format!(
                "Total time:       {} (elapsed {})\n",
                fmt_dur(t),
                fmt_dur(e)
            )),
            (Some(t), None) => o.push_str(&format!("Total time:       {}\n", fmt_dur(t))),
            (None, Some(e)) => o.push_str(&format!("Elapsed time:     {}\n", fmt_dur(e))),
            (None, None) => {}
        }
        if let Some(c) = s.total_calories {
            o.push_str(&format!("Calories:         {c} kcal\n"));
        }
        if s.avg_speed_mps.is_some() || s.max_speed_mps.is_some() {
            o.push_str(&format!(
                "Speed:            avg {}  max {} (km/h)\n",
                opt_kmh(s.avg_speed_mps),
                opt_kmh(s.max_speed_mps)
            ));
        }
        if s.avg_hr.is_some() || s.max_hr.is_some() {
            o.push_str(&format!(
                "Heart rate:       avg {}  max {} bpm\n",
                opt_u(s.avg_hr),
                opt_u(s.max_hr)
            ));
        }
        if s.avg_power.is_some() || s.max_power.is_some() {
            o.push_str(&format!(
                "Power:            avg {}  max {} W\n",
                opt_u(s.avg_power),
                opt_u(s.max_power)
            ));
        }
        if s.total_ascent_m.is_some() || s.total_descent_m.is_some() {
            o.push_str(&format!(
                "Elevation:        +{} m / -{} m\n",
                opt_u(s.total_ascent_m),
                opt_u(s.total_descent_m)
            ));
        }
    }
    o
}

fn render_csv(dec: &Decoded) -> String {
    let mut o = String::new();
    o.push_str("timestamp,latitude,longitude,altitude_m,distance_m,speed_mps,heart_rate,cadence,power\n");
    for r in &dec.records {
        let f = [
            r.unix_time.map(fmt_iso).unwrap_or_default(),
            r.lat_deg.map(|v| format!("{v:.7}")).unwrap_or_default(),
            r.lon_deg.map(|v| format!("{v:.7}")).unwrap_or_default(),
            r.altitude_m.map(|v| format!("{v:.1}")).unwrap_or_default(),
            r.distance_m.map(|v| format!("{v:.2}")).unwrap_or_default(),
            r.speed_mps.map(|v| format!("{v:.3}")).unwrap_or_default(),
            r.heart_rate.map(|v| v.to_string()).unwrap_or_default(),
            r.cadence.map(|v| v.to_string()).unwrap_or_default(),
            r.power.map(|v| v.to_string()).unwrap_or_default(),
        ];
        o.push_str(&f.join(","));
        o.push('\n');
    }
    o
}

fn render_gpx(dec: &Decoded) -> String {
    let pts: Vec<&RecordPoint> = dec
        .records
        .iter()
        .filter(|r| r.lat_deg.is_some() && r.lon_deg.is_some())
        .collect();
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    o.push_str("<gpx version=\"1.1\" creator=\"fit-file-decoder\" xmlns=\"http://www.topografix.com/GPX/1/1\" xmlns:gpxtpx=\"http://www.garmin.com/xmlschemas/TrackPointExtension/v1\">\n");
    o.push_str("  <trk>\n");
    o.push_str("    <trkseg>\n");
    for r in pts {
        o.push_str(&format!(
            "      <trkpt lat=\"{:.7}\" lon=\"{:.7}\">\n",
            r.lat_deg.unwrap(),
            r.lon_deg.unwrap()
        ));
        if let Some(a) = r.altitude_m {
            o.push_str(&format!("        <ele>{a:.1}</ele>\n"));
        }
        if let Some(t) = r.unix_time {
            o.push_str(&format!("        <time>{}</time>\n", fmt_iso(t)));
        }
        let hr = r.heart_rate;
        let cad = r.cadence;
        let pw = r.power;
        if hr.is_some() || cad.is_some() || pw.is_some() {
            o.push_str("        <extensions>\n");
            if hr.is_some() || cad.is_some() {
                o.push_str("          <gpxtpx:TrackPointExtension>\n");
                if let Some(h) = hr {
                    o.push_str(&format!("            <gpxtpx:hr>{h}</gpxtpx:hr>\n"));
                }
                if let Some(c) = cad {
                    o.push_str(&format!("            <gpxtpx:cad>{c}</gpxtpx:cad>\n"));
                }
                o.push_str("          </gpxtpx:TrackPointExtension>\n");
            }
            if let Some(p) = pw {
                o.push_str(&format!("          <power>{p}</power>\n"));
            }
            o.push_str("        </extensions>\n");
        }
        o.push_str("      </trkpt>\n");
    }
    o.push_str("    </trkseg>\n");
    o.push_str("  </trk>\n");
    o.push_str("</gpx>\n");
    o
}

fn opt_kmh(v: Option<f64>) -> String {
    v.map(|s| format!("{:.1}", s * 3.6))
        .unwrap_or_else(|| "—".into())
}
fn opt_u(v: Option<u32>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "—".into())
}

fn sport_name(code: u32) -> String {
    let name = match code {
        0 => "generic",
        1 => "running",
        2 => "cycling",
        3 => "transition",
        4 => "fitness equipment",
        5 => "swimming",
        6 => "basketball",
        7 => "soccer",
        8 => "tennis",
        9 => "american football",
        10 => "training",
        11 => "walking",
        12 => "cross-country skiing",
        13 => "alpine skiing",
        14 => "snowboarding",
        15 => "rowing",
        16 => "mountaineering",
        17 => "hiking",
        18 => "multisport",
        19 => "paddling",
        _ => return format!("sport {code}"),
    };
    name.to_string()
}

// ---------------------------------------------------------------------------
// Time formatting (no chrono — pure civil-from-days)
// ---------------------------------------------------------------------------

fn fmt_iso(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let hh = secs / 3600;
    let mm = (secs % 3600) / 60;
    let ss = secs % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Howard Hinnant's civil_from_days: days since 1970-01-01 → (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

fn fmt_dur(secs: f64) -> String {
    let total = secs.max(0.0).round() as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

// ---------------------------------------------------------------------------
// base64 decode (standard + URL-safe alphabet, whitespace tolerant)
// ---------------------------------------------------------------------------

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> i16 {
        match c {
            b'A'..=b'Z' => (c - b'A') as i16,
            b'a'..=b'z' => (c - b'a' + 26) as i16,
            b'0'..=b'9' => (c - b'0' + 52) as i16,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => -1,
        }
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3 + 3);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        if c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v < 0 {
            return Err(format!(
                "invalid base64 input (unexpected character '{}') — paste the base64 encoding of a .fit file",
                c as char
            ));
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real, self-consistent FIT activity (3 GPS records + a cycling session), generated with a
    // matching FIT CRC. See docs/checks/2026-07-23-improve-fit-file-decoder-competitor-analysis.md.
    const SAMPLE: &str = "DhBcCMAAAAAuRklUz5dAAAAUAAn9BIYABIUBBIUCAoQFBIYGAoQDAQIEAQIHAoQAAMtOQBzHcRxVVVW1BCkAAAAAiBN4UJYAADzLTkC39XEcVVVVtTYpXCsAAHAXjFXIAAB4y05AUSRyHLsmVbVoKahhAABYG5ZY+gBBAAASAA79BIYFAQACBIYHBIYIBIYJBIYLAoQOAoQPAoQQAQIRAQIUAoQVAoQWAoQBeMtOQAIAy05AwNQBAMDUAQCoYQAAKgBwF1gbiZbIAPoAFAAPJg==";

    fn decode() -> Decoded {
        parse_fit(&b64_decode(SAMPLE).unwrap()).unwrap()
    }

    #[test]
    fn parses_header_and_records() {
        let d = decode();
        assert_eq!(d.protocol_major, 1);
        assert_eq!(d.records.len(), 3);
        let r0 = &d.records[0];
        assert!((r0.lat_deg.unwrap() - 40.0).abs() < 1e-6, "{:?}", r0.lat_deg);
        assert!((r0.lon_deg.unwrap() + 105.0).abs() < 1e-6, "{:?}", r0.lon_deg);
        assert!((r0.altitude_m.unwrap() - 1600.0).abs() < 0.01);
        assert_eq!(r0.heart_rate, Some(120));
        assert_eq!(r0.cadence, Some(80));
        assert_eq!(r0.power, Some(150));
        assert_eq!(r0.unix_time, Some(1_709_971_200));
    }

    #[test]
    fn summary_has_sport_and_totals() {
        let out = render(&decode(), Format::Summary);
        assert!(out.contains("Records decoded:  3"), "{out}");
        assert!(out.contains("Sport:            cycling"), "{out}");
        assert!(out.contains("Total distance:   0.25 km"), "{out}");
        assert!(out.contains("2024-03-09T08:00:00Z"), "{out}");
    }

    #[test]
    fn csv_has_header_and_rows() {
        let out = render(&decode(), Format::Csv);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 4); // header + 3 records
        assert!(lines[0].starts_with("timestamp,latitude,longitude"));
        assert_eq!(
            lines[1],
            "2024-03-09T08:00:00Z,40.0000000,-105.0000000,1600.0,0.00,5.000,120,80,150"
        );
    }

    #[test]
    fn gpx_is_well_formed_track() {
        let out = render(&decode(), Format::Gpx);
        assert!(out.contains("<gpx version=\"1.1\""));
        assert_eq!(out.matches("<trkpt").count(), 3);
        assert!(out.contains("lat=\"40.0000000\" lon=\"-105.0000000\""));
        assert!(out.contains("<gpxtpx:hr>120</gpxtpx:hr>"));
        assert!(out.contains("<power>150</power>"));
        assert!(out.contains("<time>2024-03-09T08:00:00Z</time>"));
    }

    #[test]
    fn decode_str_dispatches_formats() {
        assert!(decode_str(SAMPLE, "csv").unwrap().starts_with("timestamp,"));
        assert!(decode_str(SAMPLE, "gpx").unwrap().contains("<gpx"));
        assert!(decode_str(SAMPLE, "").unwrap().contains("FIT file summary"));
    }

    #[test]
    fn errors_are_helpful() {
        // Not a FIT file (valid base64, wrong bytes).
        let bogus = b64_decode("aGVsbG8gd29ybGQgdGhpcyBpcyBub3QgYSBmaXQgZmlsZSE=").unwrap();
        assert!(decode_bytes(&bogus, "summary")
            .unwrap_err()
            .contains("not a FIT file"));
        // Empty input.
        assert!(decode_str("", "summary").unwrap_err().contains("no input"));
        // Bad format.
        assert!(decode_str(SAMPLE, "xml")
            .unwrap_err()
            .contains("unknown format"));
        // Invalid base64.
        assert!(decode_str("@@@@", "summary")
            .unwrap_err()
            .contains("invalid base64"));
        // Too short.
        assert!(decode_bytes(&[1, 2, 3], "summary")
            .unwrap_err()
            .contains("too short"));
    }

    #[test]
    fn iso_and_duration_formatting() {
        assert_eq!(fmt_iso(1_709_971_200), "2024-03-09T08:00:00Z");
        assert_eq!(fmt_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(fmt_dur(125.0), "2:05");
        assert_eq!(fmt_dur(3725.0), "1:02:05");
    }
}
