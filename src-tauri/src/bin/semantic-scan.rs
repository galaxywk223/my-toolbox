use std::{env, fs, path::PathBuf};

use my_toolbox_lib::semantic_scan::{schema_as_json_string, scan_semantic_repo, SemanticScanConfig};

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }

    let mut root: Option<PathBuf> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut schema_out: Option<PathBuf> = None;
    let mut no_color = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = args.get(i).map(PathBuf::from);
            }
            "--json" => {
                i += 1;
                json_out = args.get(i).map(PathBuf::from);
            }
            "--schema" => {
                i += 1;
                schema_out = args.get(i).map(PathBuf::from);
            }
            "--no-color" => {
                no_color = true;
            }
            v if !v.starts_with('-') && root.is_none() => {
                root = Some(PathBuf::from(v));
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(schema_path) = schema_out {
        let schema = schema_as_json_string();
        if let Err(e) = fs::write(&schema_path, schema.as_bytes()) {
            eprintln!("写入 schema 失败: {}", e);
            std::process::exit(2);
        }
        println!("{}", schema_path.to_string_lossy());
        return;
    }

    let root = match root {
        Some(v) => v,
        None => {
            eprintln!("缺少 --root 或 repoRoot 参数");
            print_help();
            std::process::exit(2);
        }
    };

    if !root.exists() || !root.is_dir() {
        eprintln!("无效目录: {}", root.to_string_lossy());
        std::process::exit(2);
    }

    let report = match scan_semantic_repo(&root, SemanticScanConfig::default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("扫描失败: {}", e);
            std::process::exit(1);
        }
    };

    render_terminal_report(&report, no_color);

    if let Some(out) = json_out {
        let json = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
        if let Err(e) = fs::write(&out, json.as_bytes()) {
            eprintln!("写入 JSON 失败: {}", e);
            std::process::exit(2);
        }
        println!("{}", out.to_string_lossy());
    }
}

fn print_help() {
    println!(
        "semantic-scan\n\nUSAGE:\n  semantic-scan <repoRoot> [--json <path>] [--no-color]\n  semantic-scan --schema <path>\n\nOPTIONS:\n  --root <path>     指定仓库根目录（也可作为第一个位置参数）\n  --json <path>     输出 JSON 报告\n  --schema <path>   输出 JSON Schema（不执行扫描）\n  --no-color        禁用终端颜色\n  -h, --help        显示帮助\n"
    );
}

fn render_terminal_report(report: &my_toolbox_lib::semantic_scan::SemanticReport, no_color: bool) {
    println!("Monorepo:");
    for m in &report.modules {
        println!("  - {} ({})", m.name, m.path);
    }
    println!();

    for m in &report.modules {
        println!("Module: {} (Path: {})", m.name, m.path);
        if let Some(b) = &m.backend {
            println!(
                "  Backend: framework={}, rest={}, db={}, queue={}",
                b.framework, b.rest, b.db, b.queue
            );
        }
        if let Some(f) = &m.frontend {
            println!(
                "  Frontend: builder={}, vue={}, store={}, ui={}",
                f.builder,
                f.vue.map(|v| v.to_string()).unwrap_or_else(|| "None".to_string()),
                f.store,
                f.ui
            );
        }
        if !m.frameworks.is_empty() {
            println!("  Frameworks: {}", m.frameworks.join(", "));
        }
        if !m.deps.is_empty() {
            println!("  Deps: {}", m.deps.join(", "));
        }
        if let Some(g) = &m.generated {
            println!("  Generated: {} files, {:.1} kLOC ignored", g.files, g.kloc_ignored);
        }
        if let Some(a) = &m.assets {
            println!("  Assets: {} files, {} bytes", a.files, a.bytes);
        }
        if !m.languages.is_empty() {
            let top = m
                .languages
                .iter()
                .take(6)
                .map(|l| format!("{} {:.1}%", l.language, l.percent))
                .collect::<Vec<_>>()
                .join(" · ");
            println!("  Languages: {}", top);
        }
        if !m.warnings.is_empty() {
            for w in &m.warnings {
                println!("  Warning: {}", w);
            }
        }
        println!();
    }

    let ratio = report.summary.ignored_ratio * 100.0;
    let msg = format!(
        "Noise ratio: {:.2}% (ignoredSize/totalSize)",
        ratio
    );
    if no_color {
        println!("{}", msg);
        return;
    }
    if report.summary.ignored_ratio <= 0.10 {
        println!("\u{001b}[32m{}\u{001b}[0m", msg);
    } else {
        println!("\u{001b}[31m{}\u{001b}[0m", msg);
    }
}
