//! Renders the gizza-chrome header/footer to static HTML fragments the
//! tool-page generator injects via --site-config. Usage: emit_partials <out_dir>

use gizza_chrome::{footer, header, Active};
use maud::html;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "site/partials".into());
    std::fs::create_dir_all(&out).expect("mkdir partials");
    let brand = html! {
        a.tool-brand href="https://gizza.ai" {
            img src="/logo.webp" alt="gizza.ai logo";
            span { "gizza.ai" }
        }
    };
    std::fs::write(
        format!("{out}/header.html"),
        header(brand, Active::Tool).into_string(),
    )
    .expect("write header.html");
    std::fs::write(format!("{out}/footer.html"), footer().into_string())
        .expect("write footer.html");
    eprintln!("wrote {out}/header.html + footer.html");
}
