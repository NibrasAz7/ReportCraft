//! xtask — repo task runner. `cargo xtask ci` gates every commit.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_else(|| "ci".into());
    match task.as_str() {
        "ci" => ci(),
        "fmt" => run(&["fmt", "--all", "--", "--check"]),
        "test" => run(&["test", "--workspace"]),
        "clippy" => run(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]),
        "layers" => layers(),
        other => {
            eprintln!("unknown xtask '{other}' (have: ci, fmt, test, clippy, layers)");
            ExitCode::FAILURE
        }
    }
}

fn ci() -> ExitCode {
    let steps: [&[&str]; 3] =
        [&["fmt", "--all", "--", "--check"], &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], &["test", "--workspace"]];
    for args in steps {
        if run(args) != ExitCode::SUCCESS {
            return ExitCode::FAILURE;
        }
    }
    layers()
}

fn run(args: &[&str]) -> ExitCode {
    let status = Command::new("cargo").args(args).status();
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => {
            eprintln!("cargo {} failed ({s})", args.join(" "));
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("could not run cargo: {e}");
            ExitCode::FAILURE
        }
    }
}

fn layers() -> ExitCode {
    let layer = |name: &str| -> Option<u8> {
        Some(match name {
            "rc-model" | "rc_model" => 0,
            "rc-render" | "rc_render" => 1,
            "rc-engine" | "rc_engine" => 5,
            "rc-mcp" | "rc_mcp" | "ui-egui" | "rc_ui_egui" => 6,
            n if n.ends_with("-cli") || n == "reportcraft" || n == "reportcraft-web" => 7,
            _ => return None,
        })
    };
    const UI_DEPS: &[&str] = &["egui", "eframe", "winit", "wgpu", "rfd"];

    let roots = ["crates", "apps"];
    let mut ok = true;
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let manifest = entry.path().join("Cargo.toml");
            let Ok(text) = std::fs::read_to_string(&manifest) else { continue };
            let pkg = entry.file_name().to_string_lossy().to_string();
            let Some(my_layer) = layer(&pkg) else { continue };
            let mut in_deps = false;
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with('[') {
                    in_deps = t.starts_with("[dependencies") || t.starts_with("[dev-dependencies");
                }
                if !in_deps {
                    continue;
                }
                let dep = t.split('=').next().unwrap_or("").trim();
                if dep.is_empty() {
                    continue;
                }
                if let Some(dep_layer) = layer(dep) {
                    if dep_layer >= my_layer {
                        eprintln!("layers: {pkg} (L{my_layer}) may not depend on {dep} (L{dep_layer})");
                        ok = false;
                    }
                } else if my_layer < 6 && UI_DEPS.contains(&dep) {
                    eprintln!("layers: {pkg} (L{my_layer}) may not depend on UI crate {dep}");
                    ok = false;
                }
            }
        }
    }
    if ok {
        println!("layers: ok");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
