use std::path::PathBuf;

use clap::{Parser, Subcommand};
use gizza_cli::{args, render, runtime};

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
        /// Tool name (without gizza-ai/ prefix, or full gizza-ai/name)
        name: String,
        /// Positional arguments: bare values (matched to required schema fields
        /// in order) or key=value pairs.
        #[arg(trailing_var_arg = true)]
        tool_args: Vec<String>,
        /// JSON input body (mutually exclusive with positional args)
        #[arg(long = "json")]
        json: Option<String>,
        /// Print full JSON envelope instead of human-friendly output
        #[arg(long = "json-out")]
        json_out: bool,
        /// Write binary output to this path (defaults to the filename from
        /// the response envelope)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// List all available tools
    List {
        /// Print a JSON array of {name,description,parameters}
        #[arg(long = "json-out")]
        json_out: bool,
    },
    /// Describe a single tool's schema
    Describe {
        /// Tool name (short or full gizza-ai/name)
        name: String,
        /// Print the parameters JSON Schema
        #[arg(long = "json-out")]
        json_out: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::List { json_out } => {
            let rt = boot_or_die().await;
            let tools = rt.tools();
            if json_out {
                let arr: Vec<serde_json::Value> = tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&arr).unwrap());
            } else {
                // Compute column width for alignment.
                let max_short = tools.iter().map(|t| t.short.len()).max().unwrap_or(0);
                for t in tools {
                    println!("{:<width$}  {}", t.short, t.description, width = max_short);
                }
            }
        }
        Commands::Describe { name, json_out } => {
            let rt = boot_or_die().await;
            let meta = rt.tool(&name).unwrap_or_else(|| {
                eprintln!("Unknown tool: {name}");
                std::process::exit(2);
            });
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&meta.parameters).unwrap()
                );
            } else {
                println!("Name:        {}", meta.name);
                println!("Description: {}", meta.description);
                println!(
                    "Parameters:  {}",
                    serde_json::to_string_pretty(&meta.parameters).unwrap()
                );
            }
        }
        Commands::Tool {
            name,
            tool_args,
            json,
            json_out,
            out,
        } => {
            let rt = boot_or_die().await;
            let meta = rt.tool(&name).unwrap_or_else(|| {
                eprintln!("Unknown tool: {name}");
                std::process::exit(2);
            });
            let body_json =
                match args::map_args(&meta.parameters, &tool_args, json.as_deref()) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(2);
                    }
                };
            let full_name = meta.name.clone();
            let body = match rt.run_tool(&full_name, body_json).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Dispatch error: {e}");
                    std::process::exit(1);
                }
            };
            // Check for a binary data_url envelope.
            if let Some(bin) = render::extract_binary(&body) {
                let dest = out.unwrap_or_else(|| PathBuf::from(&bin.filename));
                if let Err(e) = std::fs::write(&dest, &bin.bytes) {
                    eprintln!("Failed to write {}: {e}", dest.display());
                    std::process::exit(1);
                }
                if json_out {
                    // Print the envelope but replace data_url with the written path.
                    if let Ok(mut v) =
                        serde_json::from_slice::<serde_json::Value>(&body)
                    {
                        if let Some(ui) = v.get_mut("_for_ui") {
                            if let Some(obj) = ui.as_object_mut() {
                                obj.insert(
                                    "data_url".to_string(),
                                    serde_json::Value::String(dest.display().to_string()),
                                );
                            }
                        }
                        println!("{}", serde_json::to_string_pretty(&v).unwrap());
                    } else {
                        println!("{}", String::from_utf8_lossy(&body));
                    }
                } else {
                    // Print the for_llm summary if present.
                    let rendered = render::render(&body, false);
                    println!("{}", rendered.stdout);
                    eprintln!("Written to {}", dest.display());
                }
                // exit_code from render is 0 for the human summary here; binary writes always succeed at this point.
                std::process::exit(0);
            }
            let rendered = render::render(&body, json_out);
            println!("{}", rendered.stdout);
            std::process::exit(rendered.exit_code);
        }
    }
}

async fn boot_or_die() -> runtime::ToolRuntime {
    match runtime::boot_full().await {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Boot error: {e}");
            std::process::exit(1);
        }
    }
}
