use clap::{Parser, Subcommand};
use gizza_cli::{render, runtime};

#[derive(Parser)]
#[command(name = "gizza", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Invoke a skill tool
    Tool {
        /// Tool name (without gizza-ai/ prefix)
        name: String,
        /// JSON input body
        #[arg(long = "json")]
        json: Option<String>,
        /// Print full JSON envelope instead of human-friendly output
        #[arg(long = "json-out")]
        json_out: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Tool { name, json, json_out } => {
            let args: serde_json::Value = match json.as_deref() {
                None | Some("") => serde_json::Value::Object(serde_json::Map::new()),
                Some(s) => match serde_json::from_str(s) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Invalid JSON: {e}");
                        std::process::exit(2);
                    }
                },
            };
            let rt = match runtime::boot_minimal().await {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("Boot error: {e}");
                    std::process::exit(1);
                }
            };
            let full_name = format!("gizza-ai/{name}");
            let body = match rt.run_tool(&full_name, args).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Dispatch error: {e}");
                    std::process::exit(1);
                }
            };
            let rendered = render::render(&body, json_out);
            println!("{}", rendered.stdout);
            std::process::exit(rendered.exit_code);
        }
    }
}
