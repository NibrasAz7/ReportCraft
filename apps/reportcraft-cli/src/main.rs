//! `reportcraft-cli` — Headless CLI and MCP entrypoint for ReportCraft.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use rc_engine::Session;
use serde_json::{json, Value};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<String, String> {
    let Some(first) = args.first().map(String::as_str) else {
        return Ok(usage());
    };
    match first {
        "--help" | "-h" | "help" => Ok(usage()),
        "commands" => {
            let mut s = Session::new();
            let v = s.execute("engine.list", json!({})).map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
        }
        "mcp" => {
            let mut srv = rc_mcp::Server::new();
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            srv.serve(stdin.lock(), stdout.lock()).map_err(|e| e.to_string())?;
            Ok(String::new())
        }
        "--cmd" => {
            let spec = args.get(1).ok_or_else(|| "usage: reportcraft-cli --cmd '<id> <json>'".to_string())?;
            let (id, json_part) = match spec.split_once(char::is_whitespace) {
                Some((i, j)) => (i.trim(), j.trim()),
                None => (spec.trim(), "{}"),
            };
            let params: Value = serde_json::from_str(json_part).map_err(|e| format!("invalid JSON params: {e}"))?;
            let mut s = Session::new();
            let out = s.execute(id, params).map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
        }
        "export-svg" | "export-html" => {
            let project_path = args.get(1).ok_or_else(|| format!("usage: reportcraft-cli {first} <project.reportcraft> [output]"))?;
            let mut s = Session::new();
            s.execute("project.open", json!({"path": project_path})).map_err(|e| e.to_string())?;
            let cmd = if first == "export-svg" { "report.export_svg" } else { "report.export_html" };
            let params = match args.get(2) {
                Some(out_path) => json!({"path": out_path}),
                None => json!({}),
            };
            let out = s.execute(cmd, params).map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
        }
        other => Err(format!("unknown subcommand '{other}'\n\n{}", usage())),
    }
}

fn usage() -> String {
    [
        "reportcraft-cli — Headless BI engine, report renderer, and MCP server",
        "",
        "USAGE:",
        "  reportcraft-cli commands",
        "  reportcraft-cli --cmd '<command-id> <json-params>'",
        "  reportcraft-cli export-svg <project.reportcraft> [out.svg]",
        "  reportcraft-cli export-html <project.reportcraft> [out.html]",
        "  reportcraft-cli mcp",
    ]
    .join("\n")
}
