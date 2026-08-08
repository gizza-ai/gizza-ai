use gizza_ai_json_log_formatter_core::format_logs;

const LOGS: &str = "{\"time\":\"2026-08-08T12:00:00Z\",\"level\":\"info\",\"msg\":\"server started\",\"port\":8080}\n{\"time\":\"2026-08-08T12:00:09Z\",\"level\":\"error\",\"msg\":\"db timeout\",\"req\":{\"method\":\"GET\",\"url\":\"/api\"}}";

fn show(label: &str, r: Result<String, String>) {
    println!("=== {label} ===");
    match r {
        Ok(s) => println!("{s}"),
        Err(e) => println!("ERR: {e}"),
    }
    println!("=== end {label} ===");
}

fn main() {
    show(
        "pretty",
        format_logs(LOGS, "", "", "", "", "", "", "", "", true, "", 200, "pretty"),
    );
    show(
        "exact-pretty",
        format_logs(LOGS, "", "req.method", "GET", "exact", "", "", "", "", true, "", 200, "pretty"),
    );
    show(
        "exact-pretty-lowercase",
        format_logs(LOGS, "", "req.method", "get", "exact", "", "", "", "", true, "", 200, "pretty"),
    );
    show(
        "table-nofilter",
        format_logs(LOGS, "", "", "", "", "time,level,msg,req.method", "", "", "", true, "", 200, "table"),
    );
    show(
        "table-exact",
        format_logs(LOGS, "", "req.method", "GET", "exact", "time,level,msg,req.method", "", "", "", true, "", 200, "table"),
    );
    show(
        "csv",
        format_logs(LOGS, "", "", "", "", "time,level,msg,req.method", "", "", "", true, "", 200, "csv"),
    );
    show(
        "limit-5001",
        format_logs(LOGS, "", "", "", "", "", "", "", "", true, "", 5001, "pretty"),
    );
    show(
        "limit-5000",
        format_logs(LOGS, "", "", "", "", "", "", "", "", true, "", 5000, "pretty"),
    );
    show(
        "limit-1",
        format_logs(LOGS, "", "", "", "", "", "", "", "", true, "", 1, "pretty"),
    );
    show(
        "noflatten",
        format_logs(
            "{\"time\":\"2026-08-08T12:00:00Z\",\"level\":\"info\",\"msg\":\"server started\",\"req\":{\"method\":\"GET\"}}",
            "", "", "", "", "", "", "", "", false, "", 1, "pretty",
        ),
    );
}
