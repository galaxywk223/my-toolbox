use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use chrono::Utc;
use ignore::WalkBuilder;
use rayon::prelude::*;
use reqwest::header::USER_AGENT;
use std::io::Cursor;
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LanguageStat {
    pub language: String,
    pub bytes: u64,
    pub files: u64,
    pub percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackendStack {
    pub framework: String,
    pub rest: bool,
    pub db: String,
    pub queue: String,
    pub orm: Option<String>,
    pub migrations: Option<String>,
    pub ai_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendStack {
    pub builder: String,
    pub vue: Option<u8>,
    pub store: String,
    pub ui: String,
    pub visualization: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedSummary {
    pub files: u64,
    pub kloc_ignored: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetSummary {
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredFile {
    pub path: String,
    pub size: u64,
    pub reason: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModuleReport {
    pub name: String,
    pub path: String,
    pub languages: Vec<LanguageStat>,
    pub frameworks: Vec<String>,
    pub deps: Vec<String>,
    pub backend: Option<BackendStack>,
    pub frontend: Option<FrontendStack>,
    pub generated: Option<GeneratedSummary>,
    pub assets: Option<AssetSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSummary {
    pub total_size: u64,
    pub ignored_size: u64,
    pub ignored_ratio: f32,
    pub assets_size: u64,
    pub generated_files: u64,
    pub generated_kloc_ignored: f32,
    pub effective_files: u64,
    pub effective_kloc: f32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticReport {
    pub scan_timestamp: String,
    pub repo_root: String,
    pub modules: Vec<ModuleReport>,
    pub summary: SemanticSummary,
    pub ignored_files: Vec<IgnoredFile>,
}

pub fn schema_as_json_string() -> String {
    let schema = schemars::schema_for!(SemanticReport);
    serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_string())
}

#[derive(Debug, Clone)]
pub struct SemanticScanConfig {
    pub ignore_extensions: Vec<String>,
    pub ignore_dirs: Vec<String>,
    pub max_config_bytes: u64,
    pub asset_threshold_bytes: u64,
    pub ignored_files_limit: usize,
    pub follow_requirements_depth: usize,
}

impl Default for SemanticScanConfig {
    fn default() -> Self {
        Self {
            ignore_extensions: vec![
                ".sql".to_string(),
                ".lock".to_string(),
                ".map".to_string(),
                ".log".to_string(),
                ".tmp".to_string(),
                ".bak".to_string(),
            ],
            ignore_dirs: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "__pycache__".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".venv".to_string(),
                "venv".to_string(),
            ],
            max_config_bytes: 1_048_576,
            asset_threshold_bytes: 5 * 1024 * 1024,
            ignored_files_limit: 200,
            follow_requirements_depth: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Backend,
    Frontend,
    Unknown,
}

#[derive(Debug, Clone)]
struct ModuleSpec {
    name: String,
    rel_path: String,
    abs_path: PathBuf,
    kind: ModuleKind,
}

#[derive(Debug, Clone)]
pub struct DetectContext {
    pub repo_root: PathBuf,
    pub config: SemanticScanConfig,
}

#[derive(Debug, Clone)]
pub struct ModuleContext {
    pub name: String,
    pub rel_path: String,
    pub abs_path: PathBuf,
    pub kind: ModuleKind,
}

#[derive(Debug)]
pub enum DetectError {
    Io(String),
    Parse(String),
}

pub trait Detector: Send + Sync {
    fn id(&self) -> &'static str;
    fn detect(&self, ctx: &DetectContext, module: &ModuleContext, report: &mut ModuleReport)
        -> Result<(), DetectError>;
}

pub fn scan_semantic_repo(root: &Path, config: SemanticScanConfig) -> Result<SemanticReport, String> {
    let ctx = DetectContext {
        repo_root: root.to_path_buf(),
        config,
    };
    let modules = detect_modules(&ctx);
    let detectors = build_detectors();

    let mut ignored_files: Vec<IgnoredFile> = Vec::new();
    let mut summary = SemanticSummary {
        total_size: 0,
        ignored_size: 0,
        ignored_ratio: 0.0,
        assets_size: 0,
        generated_files: 0,
        generated_kloc_ignored: 0.0,
        effective_files: 0,
        effective_kloc: 0.0,
        warnings: Vec::new(),
    };

    let mut module_reports = Vec::new();
    for m in modules {
        let module_ctx = ModuleContext {
            name: m.name.clone(),
            rel_path: m.rel_path.clone(),
            abs_path: m.abs_path.clone(),
            kind: m.kind,
        };

        let mut report = ModuleReport {
            name: m.name.clone(),
            path: m.rel_path.clone(),
            languages: Vec::new(),
            frameworks: Vec::new(),
            deps: Vec::new(),
            backend: None,
            frontend: None,
            generated: None,
            assets: None,
            warnings: Vec::new(),
        };

        for d in &detectors {
            if let Err(e) = d.detect(&ctx, &module_ctx, &mut report) {
                report.warnings.push(format!("{}: {}", d.id(), describe_error(e)));
            }
        }

        let module_stats =
            scan_module_files(&ctx, &module_ctx).map_err(|e| format!("扫描模块失败: {}", e))?;
        report.languages = module_stats.languages;
        report.generated = module_stats.generated;
        report.assets = module_stats.assets;

        summary.total_size = summary.total_size.saturating_add(module_stats.total_size);
        summary.ignored_size = summary.ignored_size.saturating_add(module_stats.ignored_size);
        summary.assets_size = summary.assets_size.saturating_add(module_stats.assets_size);
        summary.generated_files = summary
            .generated_files
            .saturating_add(module_stats.generated_files);
        summary.generated_kloc_ignored += module_stats.generated_kloc_ignored;
        summary.effective_files = summary
            .effective_files
            .saturating_add(module_stats.effective_files);

        for f in module_stats.ignored_files {
            if ignored_files.len() >= ctx.config.ignored_files_limit {
                break;
            }
            ignored_files.push(f);
        }

        finalize_module_report(&mut report);

        report.frameworks.sort();
        report.frameworks.dedup();
        report.deps.sort();
        report.deps.dedup();

        module_reports.push(report);
    }

    if summary.total_size > 0 {
        summary.ignored_ratio = (summary.ignored_size as f64 / summary.total_size as f64) as f32;
    }

    Ok(SemanticReport {
        scan_timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        repo_root: root.to_string_lossy().to_string(),
        modules: module_reports,
        summary,
        ignored_files,
    })
}

fn describe_error(e: DetectError) -> String {
    match e {
        DetectError::Io(v) => v,
        DetectError::Parse(v) => v,
    }
}

fn build_detectors() -> Vec<Box<dyn Detector>> {
    vec![
        Box::new(DjangoDetector {}),
        Box::new(PythonDepsDetector {}),
        Box::new(MicroFrameworkDetector {}),
        Box::new(OrmMigrationDetector {}),
        Box::new(AiFeatureDetector {}),
        Box::new(RustDetector {}),
        Box::new(FlutterDetector {}),
        Box::new(AndroidDetector {}),
        Box::new(IosDetector {}),
        Box::new(ViteDetector {}),
        Box::new(PackageJsonDepsDetector {}),
        Box::new(FrontendVisDetector {}),
    ]
}

fn finalize_module_report(report: &mut ModuleReport) {
    if let Some(backend) = report.backend.as_mut() {
        if backend.framework == "Django" {
            backend.orm = Some("Django ORM".to_string());
            backend.migrations = Some("Django Migrations".to_string());
        }

        let has_signals = backend.rest
            || backend.db != "None"
            || backend.queue != "None"
            || backend.orm.is_some()
            || backend.migrations.is_some()
            || !backend.ai_features.is_empty();
        if backend.framework == "Unknown" && has_signals {
            backend.framework = "Python App (Generic)".to_string();
        }

        if let Some(v) = &backend.orm {
            report.deps.push(v.clone());
        }
        if let Some(v) = &backend.migrations {
            report.deps.push(v.clone());
        }
        for f in &backend.ai_features {
            report.deps.push(f.clone());
        }
    }
    if let Some(frontend) = report.frontend.as_mut() {
        for v in &frontend.visualization {
            report.deps.push(v.clone());
        }
    }
}

fn detect_modules(ctx: &DetectContext) -> Vec<ModuleSpec> {
    let root = &ctx.repo_root;

    let tauri_core = root.join("src-tauri");
    if tauri_core.is_dir() {
        let src = root.join("src");
        let (frontend_rel, frontend_abs) = if src.is_dir() {
            ("/src".to_string(), src)
        } else {
            ("/".to_string(), root.to_path_buf())
        };
        return vec![
            ModuleSpec {
                name: "Tauri Core".to_string(),
                rel_path: "/src-tauri".to_string(),
                abs_path: tauri_core,
                kind: ModuleKind::Backend,
            },
            ModuleSpec {
                name: "Frontend".to_string(),
                rel_path: frontend_rel,
                abs_path: frontend_abs,
                kind: ModuleKind::Frontend,
            },
        ];
    }

    let pubspec = root.join("pubspec.yaml");
    let flutter_lib = root.join("lib");
    let flutter_main = flutter_lib.join("main.dart");
    if pubspec.exists() && (flutter_lib.is_dir() || flutter_main.exists()) {
        return vec![ModuleSpec {
            name: "Flutter App".to_string(),
            rel_path: "/".to_string(),
            abs_path: root.to_path_buf(),
            kind: ModuleKind::Frontend,
        }];
    }

    let backend = root.join("backend");
    let frontend = root.join("frontend");
    if backend.is_dir() && frontend.is_dir() {
        return vec![
            ModuleSpec {
                name: "Backend".to_string(),
                rel_path: "/backend".to_string(),
                abs_path: backend,
                kind: ModuleKind::Backend,
            },
            ModuleSpec {
                name: "Frontend".to_string(),
                rel_path: "/frontend".to_string(),
                abs_path: frontend,
                kind: ModuleKind::Frontend,
            },
        ];
    }
    if backend.is_dir() {
        return vec![ModuleSpec {
            name: "Backend".to_string(),
            rel_path: "/backend".to_string(),
            abs_path: backend,
            kind: ModuleKind::Backend,
        }];
    }
    if frontend.is_dir() {
        return vec![ModuleSpec {
            name: "Frontend".to_string(),
            rel_path: "/frontend".to_string(),
            abs_path: frontend,
            kind: ModuleKind::Frontend,
        }];
    }

    let server = root.join("server");
    let client = root.join("client");
    if server.is_dir() && client.is_dir() {
        return vec![
            ModuleSpec {
                name: "Backend".to_string(),
                rel_path: "/server".to_string(),
                abs_path: server,
                kind: ModuleKind::Backend,
            },
            ModuleSpec {
                name: "Frontend".to_string(),
                rel_path: "/client".to_string(),
                abs_path: client,
                kind: ModuleKind::Frontend,
            },
        ];
    }
    if server.is_dir() {
        return vec![ModuleSpec {
            name: "Backend".to_string(),
            rel_path: "/server".to_string(),
            abs_path: server,
            kind: ModuleKind::Backend,
        }];
    }
    if client.is_dir() {
        return vec![ModuleSpec {
            name: "Frontend".to_string(),
            rel_path: "/client".to_string(),
            abs_path: client,
            kind: ModuleKind::Frontend,
        }];
    }

    let apps = root.join("apps");
    let web = root.join("web");
    if apps.is_dir() && web.is_dir() {
        return vec![
            ModuleSpec {
                name: "Apps".to_string(),
                rel_path: "/apps".to_string(),
                abs_path: apps,
                kind: ModuleKind::Unknown,
            },
            ModuleSpec {
                name: "Frontend".to_string(),
                rel_path: "/web".to_string(),
                abs_path: web,
                kind: ModuleKind::Frontend,
            },
        ];
    }
    let apps_web = root.join("apps").join("web");
    if apps_web.is_dir() {
        return vec![ModuleSpec {
            name: "Frontend".to_string(),
            rel_path: "/apps/web".to_string(),
            abs_path: apps_web,
            kind: ModuleKind::Frontend,
        }];
    }

    vec![ModuleSpec {
        name: "Repo".to_string(),
        rel_path: "/".to_string(),
        abs_path: root.to_path_buf(),
        kind: ModuleKind::Unknown,
    }]
}

struct ModuleScanStats {
    total_size: u64,
    ignored_size: u64,
    assets_size: u64,
    generated_files: u64,
    generated_kloc_ignored: f32,
    effective_files: u64,
    languages: Vec<LanguageStat>,
    ignored_files: Vec<IgnoredFile>,
    generated: Option<GeneratedSummary>,
    assets: Option<AssetSummary>,
}

fn scan_module_files(ctx: &DetectContext, module: &ModuleContext) -> Result<ModuleScanStats, String> {
    let files = list_files(&module.abs_path, &ctx.config, &ctx.repo_root)?;
    let total_size: u64 = files.iter().map(|f| f.size).sum();

    let ignored_patterns = build_ignored_patterns(&ctx.config);
    let ignore_dirs_set: HashSet<String> = ctx
        .config
        .ignore_dirs
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let aggregated = files
        .par_iter()
        .fold(
            || LocalAgg::default(),
            |mut agg, f| {
                let classification =
                    classify_file(&f.rel_path, f.size, &ignored_patterns, &ignore_dirs_set, ctx);
                match classification.kind {
                    ClassifiedKind::Ignored { reason } => {
                        agg.ignored_size = agg.ignored_size.saturating_add(f.size);
                        if agg.ignored_files.len() < ctx.config.ignored_files_limit {
                            agg.ignored_files.push(IgnoredFile {
                                path: f.rel_path.clone(),
                                size: f.size,
                                reason,
                                category: "Ignored".to_string(),
                            });
                        }
                    }
                    ClassifiedKind::Asset => {
                        agg.assets_size = agg.assets_size.saturating_add(f.size);
                        agg.assets_files += 1;
                    }
                    ClassifiedKind::Generated => {
                        agg.generated_files += 1;
                        agg.generated_paths.push(f.abs_path.clone());
                    }
                    ClassifiedKind::Code { language } => {
                        agg.effective_files += 1;
                        *agg.language_bytes.entry(language.clone()).or_insert(0) += f.size;
                        *agg.language_files.entry(language).or_insert(0) += 1;
                    }
                }
                agg
            },
        )
        .reduce(|| LocalAgg::default(), |a, b| a.merge(b, ctx.config.ignored_files_limit));

    let generated_kloc_ignored = compute_generated_kloc(&aggregated.generated_paths, ctx)?;

    let languages = compute_language_percentages(aggregated.language_bytes, aggregated.language_files);

    Ok(ModuleScanStats {
        total_size,
        ignored_size: aggregated.ignored_size,
        assets_size: aggregated.assets_size,
        generated_files: aggregated.generated_files,
        generated_kloc_ignored,
        effective_files: aggregated.effective_files,
        languages: languages.clone(),
        ignored_files: aggregated.ignored_files,
        generated: if aggregated.generated_files > 0 {
            Some(GeneratedSummary {
                files: aggregated.generated_files,
                kloc_ignored: generated_kloc_ignored,
            })
        } else {
            None
        },
        assets: if aggregated.assets_files > 0 {
            Some(AssetSummary {
                files: aggregated.assets_files,
                bytes: aggregated.assets_size,
            })
        } else {
            None
        },
    })
}

#[derive(Default)]
struct LocalAgg {
    ignored_size: u64,
    assets_size: u64,
    assets_files: u64,
    generated_files: u64,
    effective_files: u64,
    language_bytes: HashMap<String, u64>,
    language_files: HashMap<String, u64>,
    ignored_files: Vec<IgnoredFile>,
    generated_paths: Vec<PathBuf>,
}

impl LocalAgg {
    fn merge(mut self, other: LocalAgg, ignored_files_limit: usize) -> LocalAgg {
        self.ignored_size = self.ignored_size.saturating_add(other.ignored_size);
        self.assets_size = self.assets_size.saturating_add(other.assets_size);
        self.assets_files = self.assets_files.saturating_add(other.assets_files);
        self.generated_files = self.generated_files.saturating_add(other.generated_files);
        self.effective_files = self.effective_files.saturating_add(other.effective_files);
        for (k, v) in other.language_bytes {
            *self.language_bytes.entry(k).or_insert(0) += v;
        }
        for (k, v) in other.language_files {
            *self.language_files.entry(k).or_insert(0) += v;
        }
        for item in other.ignored_files {
            if self.ignored_files.len() >= ignored_files_limit {
                break;
            }
            self.ignored_files.push(item);
        }
        self.generated_paths.extend(other.generated_paths);
        self
    }
}

struct FileEntry {
    rel_path: String,
    abs_path: PathBuf,
    size: u64,
}

fn list_files(root: &Path, cfg: &SemanticScanConfig, repo_root: &Path) -> Result<Vec<FileEntry>, String> {
    let mut out = Vec::new();
    let ignore_dirs_set: HashSet<String> =
        cfg.ignore_dirs.iter().map(|s| s.to_ascii_lowercase()).collect();

    let mut builder = WalkBuilder::new(root);
    builder.hidden(false).parents(true);
    let repo_canon = fs::canonicalize(repo_root).map_err(|e| format!("路径无效: {}", e))?;
    let repo_canon_for_filter = repo_canon.clone();

    let walker = builder.filter_entry(move |entry| {
        let p = entry.path();
        if p == repo_canon_for_filter.join(".git") {
            return false;
        }
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if ignore_dirs_set.contains(&name.to_ascii_lowercase()) {
                    return false;
                }
            }
        }
        true
    });

    for result in walker.build() {
        let entry = match result {
            Ok(v) => v,
            Err(_) => continue,
        };
        if !entry
            .file_type()
            .map(|t| t.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = meta.len();
        let abs = entry.path().to_path_buf();
        let rel = abs
            .strip_prefix(&repo_canon)
            .unwrap_or(&abs)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(FileEntry {
            rel_path: rel.trim_start_matches("./").to_string(),
            abs_path: abs,
            size,
        });
    }
    Ok(out)
}

fn build_ignored_patterns(cfg: &SemanticScanConfig) -> Vec<String> {
    cfg.ignore_extensions
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Clone)]
struct ClassifiedFile {
    kind: ClassifiedKind,
}

#[derive(Debug, Clone)]
enum ClassifiedKind {
    Ignored { reason: String },
    Generated,
    Asset,
    Code { language: String },
}

fn classify_file(
    rel_path: &str,
    size: u64,
    ignored_extensions: &[String],
    ignore_dirs: &HashSet<String>,
    ctx: &DetectContext,
) -> ClassifiedFile {
    let p = rel_path.replace('\\', "/");
    let lower = p.to_ascii_lowercase();

    for part in lower.split('/') {
        if ignore_dirs.contains(part) {
            return ClassifiedFile {
                kind: ClassifiedKind::Ignored {
                    reason: "ignoredDir".to_string(),
                },
            };
        }
    }

    if lower.ends_with(".min.js") {
        return ClassifiedFile {
            kind: ClassifiedKind::Ignored {
                reason: "ignoredMinified".to_string(),
            },
        };
    }
    for ext in ignored_extensions {
        if lower.ends_with(ext) {
            return ClassifiedFile {
                kind: ClassifiedKind::Ignored {
                    reason: "ignoredExtension".to_string(),
                },
            };
        }
    }

    if size > ctx.config.asset_threshold_bytes {
        return ClassifiedFile {
            kind: ClassifiedKind::Asset,
        };
    }

    if is_generated_migration(&lower) {
        return ClassifiedFile {
            kind: ClassifiedKind::Generated,
        };
    }

    ClassifiedFile {
        kind: ClassifiedKind::Code {
            language: language_from_extension(&lower),
        },
    }
}

fn is_generated_migration(lower_path: &str) -> bool {
    if !lower_path.ends_with(".py") {
        return false;
    }
    let path = Path::new(lower_path);
    let parent = match path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()) {
        Some(v) => v,
        None => return false,
    };
    parent.eq_ignore_ascii_case("migrations")
}

fn language_from_extension(lower_path: &str) -> String {
    let path = Path::new(lower_path);
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return "Other".to_string();
    };
    match ext {
        "ts" | "tsx" => "TypeScript".to_string(),
        "js" | "jsx" | "cjs" | "mjs" => "JavaScript".to_string(),
        "vue" => "Vue".to_string(),
        "dart" => "Dart".to_string(),
        "py" => "Python".to_string(),
        "rs" => "Rust".to_string(),
        "go" => "Go".to_string(),
        "java" => "Java".to_string(),
        "kt" | "kts" => "Kotlin".to_string(),
        "swift" => "Swift".to_string(),
        "m" | "mm" => "Objective-C".to_string(),
        "css" | "scss" | "sass" | "less" => "CSS".to_string(),
        "html" | "htm" => "HTML".to_string(),
        "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "plist" => "Config".to_string(),
        "md" => "Markdown".to_string(),
        _ => "Other".to_string(),
    }
}

fn compute_language_percentages(
    bytes_by_lang: HashMap<String, u64>,
    files_by_lang: HashMap<String, u64>,
) -> Vec<LanguageStat> {
    let total: u64 = bytes_by_lang.values().sum();
    let mut out: Vec<LanguageStat> = bytes_by_lang
        .into_iter()
        .map(|(language, bytes)| {
            let files = files_by_lang.get(&language).copied().unwrap_or(0);
            let percent = if total == 0 {
                0.0
            } else {
                ((bytes as f64) * 100.0 / (total as f64)) as f32
            };
            LanguageStat {
                language,
                bytes,
                files,
                percent,
            }
        })
        .collect();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out
}

fn compute_generated_kloc(paths: &[PathBuf], ctx: &DetectContext) -> Result<f32, String> {
    let mut total_lines: u64 = 0;
    let mut total_bytes: u64 = 0;
    let byte_budget = 16 * 1024 * 1024u64;

    for p in paths {
        if total_bytes >= byte_budget {
            break;
        }
        let meta = match fs::metadata(p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = meta.len();
        if size > ctx.config.asset_threshold_bytes {
            continue;
        }
        if total_bytes.saturating_add(size) > byte_budget {
            break;
        }
        let mut file = match fs::File::open(p) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_ok() {
            total_lines = total_lines.saturating_add(buf.lines().count() as u64);
            total_bytes = total_bytes.saturating_add(size);
        }
    }
    Ok((total_lines as f32) / 1000.0)
}

fn read_text_with_limit(path: &Path, max_bytes: u64) -> Result<Option<String>, DetectError> {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(DetectError::Io(format!("读取失败: {}", e)));
        }
    };
    if meta.len() > max_bytes {
        return Err(DetectError::Parse(format!(
            "文件过大（{} bytes），已跳过",
            meta.len()
        )));
    }
    let mut file = fs::File::open(path).map_err(|e| DetectError::Io(format!("读取失败: {}", e)))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| DetectError::Io(format!("读取失败: {}", e)))?;
    Ok(Some(String::from_utf8_lossy(&bytes).to_string()))
}

struct DjangoDetector;

impl Detector for DjangoDetector {
    fn id(&self) -> &'static str {
        "DjangoDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Backend {
            return Ok(());
        }
        let manage = module.abs_path.join("manage.py");
        if !manage.exists() {
            return Ok(());
        }
        if let Err(e) = read_text_with_limit(&manage, ctx.config.max_config_bytes) {
            report.warnings.push(format!("manage.py: {}", describe_error(e)));
        }
        report.frameworks.push("Django".to_string());
        report.backend = Some(BackendStack {
            framework: "Django".to_string(),
            rest: false,
            db: "None".to_string(),
            queue: "None".to_string(),
            orm: Some("Django ORM".to_string()),
            migrations: Some("Django Migrations".to_string()),
            ai_features: Vec::new(),
        });
        Ok(())
    }
}

struct PythonDepsDetector;

impl Detector for PythonDepsDetector {
    fn id(&self) -> &'static str {
        "PythonDepsDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Backend {
            return Ok(());
        }
        let mut names = Vec::new();

        let req = module.abs_path.join("requirements.txt");
        collect_python_deps_from_requirements(&req, ctx, &mut names)?;

        let pyproject = module.abs_path.join("pyproject.toml");
        collect_python_deps_from_pyproject(&pyproject, ctx, &mut names)?;

        let pipfile = module.abs_path.join("Pipfile");
        collect_python_deps_from_pipfile(&pipfile, ctx, &mut names)?;

        apply_backend_semantics(&names, report);
        Ok(())
    }
}

fn apply_backend_semantics(deps: &[String], report: &mut ModuleReport) {
    let mut backend = report.backend.clone().unwrap_or(BackendStack {
        framework: "Unknown".to_string(),
        rest: false,
        db: "None".to_string(),
        queue: "None".to_string(),
        orm: None,
        migrations: None,
        ai_features: Vec::new(),
    });

    let mut has_psycopg2 = false;
    let mut has_mysqlclient = false;
    for d in deps {
        let n = d.to_ascii_lowercase();
        if n.contains("djangorestframework") {
            backend.rest = true;
            report.frameworks.push("DRF".to_string());
        }
        if n.contains("celery") {
            backend.queue = "Celery".to_string();
            report.deps.push("Celery".to_string());
        }
        if n.contains("psycopg2") {
            has_psycopg2 = true;
        }
        if n.contains("mysqlclient") {
            has_mysqlclient = true;
        }
        if n.contains("django") {
            if backend.framework == "Unknown" {
                backend.framework = "Django".to_string();
                report.frameworks.push("Django".to_string());
            }
        }
    }

    if has_psycopg2 {
        backend.db = "PostgreSQL".to_string();
        report.deps.push("PostgreSQL".to_string());
    } else if has_mysqlclient {
        backend.db = "MySQL".to_string();
        report.deps.push("MySQL".to_string());
    }

    report.backend = Some(backend);
}

fn apply_backend_microframework(deps: &[String], backend: &mut BackendStack, report: &mut ModuleReport) {
    if backend.framework != "Unknown" && backend.framework != "Python App (Generic)" {
        return;
    }
    let mut has_fastapi = false;
    let mut has_flask = false;
    for d in deps {
        let n = d.to_ascii_lowercase();
        if n.contains("fastapi") {
            has_fastapi = true;
        }
        if n.contains("flask") || n.contains("quart") {
            has_flask = true;
        }
    }
    if has_fastapi {
        backend.framework = "FastAPI".to_string();
        report.frameworks.push("FastAPI".to_string());
    } else if has_flask {
        backend.framework = "Flask".to_string();
        report.frameworks.push("Flask".to_string());
    }
}

fn apply_backend_orm_migrations(deps: &[String], backend: &mut BackendStack, report: &mut ModuleReport) {
    if backend.framework == "Django" {
        backend.orm = Some("Django ORM".to_string());
        backend.migrations = Some("Django Migrations".to_string());
        report.deps.push("Django ORM".to_string());
        report.deps.push("Django Migrations".to_string());
        return;
    }
    let mut orm: Option<&'static str> = None;
    let mut migrations: Option<&'static str> = None;
    for d in deps {
        let n = d.to_ascii_lowercase();
        if migrations.is_none() && n.contains("alembic") {
            migrations = Some("Alembic");
        }
        if orm.is_none() && (n.contains("sqlalchemy") || n.contains("flask-sqlalchemy")) {
            orm = Some("SQLAlchemy");
        }
        if orm.is_none() && n.contains("tortoise-orm") {
            orm = Some("Tortoise");
        }
        if orm.is_none() && n.contains("peewee") {
            orm = Some("Peewee");
        }
    }
    if let Some(v) = orm {
        backend.orm = Some(v.to_string());
        report.deps.push(v.to_string());
    }
    if let Some(v) = migrations {
        backend.migrations = Some(v.to_string());
        report.deps.push(v.to_string());
    }
}

fn apply_backend_ai_features(deps: &[String], backend: &mut BackendStack, report: &mut ModuleReport) {
    let mut features = backend.ai_features.clone();
    let mut add = |s: &str| {
        if !features.iter().any(|v| v == s) {
            features.push(s.to_string());
        }
    };
    for d in deps {
        let n = d.to_ascii_lowercase();
        if n.contains("openai") {
            add("OpenAI");
        }
        if n.contains("langchain") {
            add("LangChain");
        }
        if n.contains("anthropic") {
            add("Anthropic");
        }
        if n.contains("transformers") {
            add("Transformers");
        }
        if n.contains("pytorch") {
            add("PyTorch");
        }
        if n.contains("tensorflow") {
            add("TensorFlow");
        }
    }
    if !features.is_empty() {
        backend.ai_features = features;
        for f in &backend.ai_features {
            report.deps.push(f.clone());
        }
    }
}

fn read_first_lines(path: &Path, ctx: &DetectContext, max_lines: usize) -> Result<Option<String>, DetectError> {
    let Some(raw) = read_text_with_limit(path, ctx.config.max_config_bytes)? else {
        return Ok(None);
    };
    Ok(Some(raw.lines().take(max_lines).collect::<Vec<_>>().join("\n")))
}

struct MicroFrameworkDetector;

impl Detector for MicroFrameworkDetector {
    fn id(&self) -> &'static str {
        "MicroFrameworkDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Backend {
            return Ok(());
        }

        let mut names = Vec::new();
        let req = module.abs_path.join("requirements.txt");
        collect_python_deps_from_requirements(&req, ctx, &mut names)?;
        let pyproject = module.abs_path.join("pyproject.toml");
        collect_python_deps_from_pyproject(&pyproject, ctx, &mut names)?;
        let pipfile = module.abs_path.join("Pipfile");
        collect_python_deps_from_pipfile(&pipfile, ctx, &mut names)?;

        let mut backend = report.backend.clone().unwrap_or(BackendStack {
            framework: "Unknown".to_string(),
            rest: false,
            db: "None".to_string(),
            queue: "None".to_string(),
            orm: None,
            migrations: None,
            ai_features: Vec::new(),
        });

        apply_backend_microframework(&names, &mut backend, report);

        if backend.framework == "Unknown" || backend.framework == "Python App (Generic)" {
            let init_py = module.abs_path.join("app").join("__init__.py");
            if init_py.exists() {
                if let Some(head) = read_first_lines(&init_py, ctx, 50)? {
                    if head.contains("Flask(__name__)") {
                        backend.framework = "Flask".to_string();
                        report.frameworks.push("Flask".to_string());
                    }
                }
            }
            let main1 = module.abs_path.join("main.py");
            let main2 = module.abs_path.join("app").join("main.py");
            let main_path = if main1.exists() { Some(main1) } else if main2.exists() { Some(main2) } else { None };
            if let Some(p) = main_path {
                if let Some(head) = read_first_lines(&p, ctx, 50)? {
                    if head.contains("FastAPI(") || head.contains("FastAPI()") {
                        backend.framework = "FastAPI".to_string();
                        report.frameworks.push("FastAPI".to_string());
                    }
                }
            }
        }

        report.backend = Some(backend);
        Ok(())
    }
}

struct OrmMigrationDetector;

impl Detector for OrmMigrationDetector {
    fn id(&self) -> &'static str {
        "OrmMigrationDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Backend {
            return Ok(());
        }
        let mut names = Vec::new();
        let req = module.abs_path.join("requirements.txt");
        collect_python_deps_from_requirements(&req, ctx, &mut names)?;
        let pyproject = module.abs_path.join("pyproject.toml");
        collect_python_deps_from_pyproject(&pyproject, ctx, &mut names)?;
        let pipfile = module.abs_path.join("Pipfile");
        collect_python_deps_from_pipfile(&pipfile, ctx, &mut names)?;

        if module.abs_path.join("migrations").join("alembic.ini").exists() {
            names.push("alembic".to_string());
        }

        let mut backend = report.backend.clone().unwrap_or(BackendStack {
            framework: "Unknown".to_string(),
            rest: false,
            db: "None".to_string(),
            queue: "None".to_string(),
            orm: None,
            migrations: None,
            ai_features: Vec::new(),
        });
        apply_backend_orm_migrations(&names, &mut backend, report);
        report.backend = Some(backend);
        Ok(())
    }
}

struct AiFeatureDetector;

impl Detector for AiFeatureDetector {
    fn id(&self) -> &'static str {
        "AiFeatureDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Backend {
            return Ok(());
        }
        let mut names = Vec::new();
        let req = module.abs_path.join("requirements.txt");
        collect_python_deps_from_requirements(&req, ctx, &mut names)?;
        let pyproject = module.abs_path.join("pyproject.toml");
        collect_python_deps_from_pyproject(&pyproject, ctx, &mut names)?;
        let pipfile = module.abs_path.join("Pipfile");
        collect_python_deps_from_pipfile(&pipfile, ctx, &mut names)?;

        let mut backend = report.backend.clone().unwrap_or(BackendStack {
            framework: "Unknown".to_string(),
            rest: false,
            db: "None".to_string(),
            queue: "None".to_string(),
            orm: None,
            migrations: None,
            ai_features: Vec::new(),
        });
        apply_backend_ai_features(&names, &mut backend, report);

        let mut has_custom = false;
        let ignore_dirs: HashSet<String> = ctx
            .config
            .ignore_dirs
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let mut builder = WalkBuilder::new(&module.abs_path);
        builder.hidden(false).parents(true);
        let walker = builder.filter_entry(move |entry| {
            let p = entry.path();
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if ignore_dirs.contains(&name.to_ascii_lowercase()) {
                        return false;
                    }
                }
            }
            true
        });

        for r in walker.build() {
            let entry = match r {
                Ok(v) => v,
                Err(_) => continue,
            };
            let p = entry.path();
            let s = p.to_string_lossy().to_ascii_lowercase().replace('\\', "/");
            if s.contains("/ai/") || s.contains("/llm/") || s.contains("/ml/") || s.contains("ai_planner") || s.contains("prompts") {
                has_custom = true;
                break;
            }
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                let n = name.to_ascii_lowercase();
                if n == "llm_client.py" || n == "prompts.py" {
                    has_custom = true;
                    break;
                }
            }
        }

        if has_custom {
            if !backend.ai_features.iter().any(|v| v == "Custom AI Logic") {
                backend.ai_features.push("Custom AI Logic".to_string());
                report.deps.push("Custom AI Logic".to_string());
            }
        }

        report.backend = Some(backend);
        Ok(())
    }
}

fn collect_cargo_dep_keys(path: &Path, ctx: &DetectContext, out: &mut Vec<String>) -> Result<(), DetectError> {
    let Some(raw) = (match read_text_with_limit(path, ctx.config.max_config_bytes) {
        Ok(v) => v,
        Err(e) => return Err(e),
    }) else {
        return Ok(());
    };
    let parsed: toml::Value = raw
        .parse::<toml::Value>()
        .map_err(|e| DetectError::Parse(format!("Cargo.toml 解析失败: {}", e)))?;
    let deps = parsed.get("dependencies").and_then(|d| d.as_table());
    if let Some(table) = deps {
        for (k, _) in table {
            out.push(k.to_ascii_lowercase());
        }
    }
    Ok(())
}

struct RustDetector;

impl Detector for RustDetector {
    fn id(&self) -> &'static str {
        "RustDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Backend {
            return Ok(());
        }

        let mut cargo = module.abs_path.join("Cargo.toml");
        if !cargo.exists()
            && module
                .abs_path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|v| v.eq_ignore_ascii_case("src-tauri"))
        {
            if let Some(parent) = module.abs_path.parent() {
                let alt = parent.join("Cargo.toml");
                if alt.exists() {
                    cargo = alt;
                }
            }
        }
        if !cargo.exists() {
            return Ok(());
        }

        let mut deps = Vec::new();
        collect_cargo_dep_keys(&cargo, ctx, &mut deps)?;

        let has_tauri_conf = module.abs_path.join("tauri.conf.json").exists();
        let mut backend = report.backend.clone().unwrap_or(BackendStack {
            framework: "Unknown".to_string(),
            rest: false,
            db: "None".to_string(),
            queue: "None".to_string(),
            orm: None,
            migrations: None,
            ai_features: Vec::new(),
        });

        let can_set_framework = backend.framework == "Unknown" || backend.framework == "Python App (Generic)";
        if can_set_framework {
            let fw = if deps.iter().any(|d| d == "tauri") || has_tauri_conf {
                Some("Tauri")
            } else if deps.iter().any(|d| d == "actix-web") {
                Some("Actix Web")
            } else if deps.iter().any(|d| d == "axum") {
                Some("Axum")
            } else if deps.iter().any(|d| d == "rocket") {
                Some("Rocket")
            } else {
                None
            };
            if let Some(v) = fw {
                backend.framework = v.to_string();
                report.frameworks.push(v.to_string());
            }
        }

        if backend.orm.is_none() {
            let orm = if deps.iter().any(|d| d == "diesel") {
                Some("Diesel")
            } else if deps.iter().any(|d| d == "sea-orm") {
                Some("SeaORM")
            } else if deps.iter().any(|d| d == "sqlx") {
                Some("SQLx")
            } else {
                None
            };
            if let Some(v) = orm {
                backend.orm = Some(v.to_string());
                report.deps.push(v.to_string());
            }
        }

        if backend.db == "None" {
            let db = if deps.iter().any(|d| d == "postgres" || d == "tokio-postgres") {
                Some("PostgreSQL")
            } else if deps.iter().any(|d| d == "mysql" || d == "mysql_async") {
                Some("MySQL")
            } else if deps.iter().any(|d| d == "rusqlite") {
                Some("SQLite")
            } else {
                None
            };
            if let Some(v) = db {
                backend.db = v.to_string();
                report.deps.push(v.to_string());
            }
        }

        report.backend = Some(backend);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PubspecSection {
    None,
    Environment,
    Dependencies,
    DevDependencies,
}

fn parse_pubspec(raw: &str) -> (HashSet<String>, Option<String>) {
    let mut section = PubspecSection::None;
    let mut deps: HashSet<String> = HashSet::new();
    let mut sdk: Option<String> = None;

    for line in raw.lines() {
        let line = line.trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len().saturating_sub(trimmed.len());
        if indent == 0 && trimmed.ends_with(':') {
            section = match trimmed.trim_end_matches(':') {
                "environment" => PubspecSection::Environment,
                "dependencies" => PubspecSection::Dependencies,
                "dev_dependencies" => PubspecSection::DevDependencies,
                _ => PubspecSection::None,
            };
            continue;
        }

        match section {
            PubspecSection::Environment => {
                if sdk.is_none() && trimmed.starts_with("sdk:") {
                    let v = trimmed
                        .trim_start_matches("sdk:")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .trim()
                        .to_string();
                    if !v.is_empty() {
                        sdk = Some(v);
                    }
                }
            }
            PubspecSection::Dependencies | PubspecSection::DevDependencies => {
                if indent < 2 {
                    continue;
                }
                let Some((key, _rest)) = trimmed.split_once(':') else {
                    continue;
                };
                let key = key.trim();
                if key.is_empty() || key.starts_with('-') {
                    continue;
                }
                deps.insert(key.to_ascii_lowercase());
            }
            PubspecSection::None => {}
        }
    }

    (deps, sdk)
}

fn normalize_flutter_dep_key(key: &str) -> String {
    key.trim().to_ascii_lowercase().replace('-', "_")
}

struct FlutterDetector;

impl Detector for FlutterDetector {
    fn id(&self) -> &'static str {
        "FlutterDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Frontend {
            return Ok(());
        }
        let pubspec = module.abs_path.join("pubspec.yaml");
        if !pubspec.exists() {
            return Ok(());
        }

        let Some(raw) = read_text_with_limit(&pubspec, ctx.config.max_config_bytes)? else {
            return Ok(());
        };
        let (deps, sdk) = parse_pubspec(&raw);

        if let Some(r) = sdk {
            report.deps.push(format!("Dart SDK {}", r));
        }

        let is_flutter = deps.iter().any(|d| d == "flutter");
        if !is_flutter {
            return Ok(());
        }

        report.frameworks.push("Flutter".to_string());
        report
            .frameworks
            .push("Mobile / Cross-platform".to_string());

        let mut frontend = report.frontend.clone().unwrap_or(FrontendStack {
            builder: "Unknown".to_string(),
            vue: None,
            store: "None".to_string(),
            ui: "None".to_string(),
            visualization: Vec::new(),
        });
        if frontend.builder == "Unknown" {
            frontend.builder = "Flutter".to_string();
        }

        let mut set_store = |v: &str| {
            if frontend.store == "None" {
                frontend.store = v.to_string();
            }
        };

        let mut add_dep = |label: &str| {
            report.deps.push(label.to_string());
        };

        let mut has_provider = false;
        let mut has_riverpod = false;
        let mut has_bloc = false;
        let mut has_get = false;

        for d in deps {
            let n = normalize_flutter_dep_key(&d);
            match n.as_str() {
                "provider" => {
                    has_provider = true;
                    add_dep("Provider");
                }
                "flutter_riverpod" | "riverpod" => {
                    has_riverpod = true;
                    add_dep("Riverpod");
                }
                "flutter_bloc" | "bloc" => {
                    has_bloc = true;
                    add_dep("Bloc");
                }
                "get" => {
                    has_get = true;
                    add_dep("GetX");
                }
                "isar" => add_dep("Isar"),
                "hive" => add_dep("Hive"),
                "sqflite" => add_dep("Sqflite"),
                "shared_preferences" => add_dep("SharedPreferences"),
                "dio" => add_dep("Dio"),
                "http" => add_dep("HTTP"),
                "go_router" => add_dep("GoRouter"),
                "auto_route" => add_dep("AutoRoute"),
                _ => {}
            }
        }

        if has_riverpod {
            set_store("Riverpod");
        } else if has_bloc {
            set_store("Bloc");
        } else if has_provider {
            set_store("Provider");
        } else if has_get {
            set_store("GetX");
        }

        report.frontend = Some(frontend);
        Ok(())
    }
}

fn extract_first_int(s: &str) -> Option<u32> {
    let mut digits = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !digits.is_empty() {
            break;
        }
    }
    digits.parse::<u32>().ok()
}

fn extract_gradle_sdk_versions(raw: &str) -> (Option<u32>, Option<u32>) {
    let mut min_sdk: Option<u32> = None;
    let mut target_sdk: Option<u32> = None;
    for line in raw.lines().take(200) {
        if min_sdk.is_none() {
            if let Some(idx) = line.find("minSdkVersion") {
                min_sdk = extract_first_int(&line[idx + "minSdkVersion".len()..]);
            } else if let Some(idx) = line.find("minSdk") {
                min_sdk = extract_first_int(&line[idx + "minSdk".len()..]);
            }
        }
        if target_sdk.is_none() {
            if let Some(idx) = line.find("targetSdkVersion") {
                target_sdk = extract_first_int(&line[idx + "targetSdkVersion".len()..]);
            } else if let Some(idx) = line.find("targetSdk") {
                target_sdk = extract_first_int(&line[idx + "targetSdk".len()..]);
            }
        }
        if min_sdk.is_some() && target_sdk.is_some() {
            break;
        }
    }
    (min_sdk, target_sdk)
}

struct AndroidDetector;

impl Detector for AndroidDetector {
    fn id(&self) -> &'static str {
        "AndroidDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Frontend {
            return Ok(());
        }
        if !module.abs_path.join("pubspec.yaml").exists() {
            return Ok(());
        }
        let gradle = module.abs_path.join("android").join("build.gradle");
        let gradle_kts = module.abs_path.join("android").join("build.gradle.kts");
        let path = if gradle.exists() {
            Some(gradle)
        } else if gradle_kts.exists() {
            Some(gradle_kts)
        } else {
            None
        };
        let Some(path) = path else {
            return Ok(());
        };

        report.deps.push("Android".to_string());

        if let Some(raw) = read_text_with_limit(&path, ctx.config.max_config_bytes)? {
            let (min_sdk, target_sdk) = extract_gradle_sdk_versions(&raw);
            if let Some(v) = min_sdk {
                report.deps.push(format!("Android minSdk={}", v));
            }
            if let Some(v) = target_sdk {
                report.deps.push(format!("Android targetSdk={}", v));
            }
        }
        Ok(())
    }
}

struct IosDetector;

impl Detector for IosDetector {
    fn id(&self) -> &'static str {
        "IosDetector"
    }

    fn detect(
        &self,
        _ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Frontend {
            return Ok(());
        }
        if !module.abs_path.join("pubspec.yaml").exists() {
            return Ok(());
        }
        let ios = module.abs_path.join("ios");
        if !ios.is_dir() {
            return Ok(());
        }
        if ios.join("Podfile").exists() {
            report.deps.push("iOS".to_string());
            return Ok(());
        }
        if let Ok(rd) = fs::read_dir(&ios) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()).is_some_and(|v| v == "xcodeproj") {
                    report.deps.push("iOS".to_string());
                    break;
                }
            }
        }
        Ok(())
    }
}

struct FrontendVisDetector;

impl Detector for FrontendVisDetector {
    fn id(&self) -> &'static str {
        "FrontendVisDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Frontend {
            return Ok(());
        }
        let mut package_json = module.abs_path.join("package.json");
        if !package_json.exists()
            && module
                .abs_path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|v| v.eq_ignore_ascii_case("src"))
        {
            if let Some(parent) = module.abs_path.parent() {
                let alt = parent.join("package.json");
                if alt.exists() {
                    package_json = alt;
                }
            }
        }
        let Some(raw) = (match read_text_with_limit(&package_json, ctx.config.max_config_bytes) {
            Ok(v) => v,
            Err(e) => {
                report.warnings.push(describe_error(e));
                return Ok(());
            }
        }) else {
            return Ok(());
        };
        let json: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                report.warnings.push(format!("package.json 解析失败: {}", e));
                return Ok(());
            }
        };
        let deps = json.get("dependencies").and_then(|d| d.as_object());
        let Some(deps) = deps else {
            return Ok(());
        };

        let mut frontend = report.frontend.clone().unwrap_or(FrontendStack {
            builder: "Unknown".to_string(),
            vue: None,
            store: "None".to_string(),
            ui: "None".to_string(),
            visualization: Vec::new(),
        });
        let mut add = |name: &str| {
            if !frontend.visualization.iter().any(|v| v == name) {
                frontend.visualization.push(name.to_string());
            }
            report.deps.push(name.to_string());
        };
        if deps.contains_key("echarts") {
            add("ECharts");
        }
        if deps.contains_key("vue-echarts") {
            add("vue-echarts");
        }
        if deps.contains_key("chart.js") {
            add("Chart.js");
        }
        if deps.contains_key("d3") {
            add("D3");
        }
        if deps.contains_key("highcharts") {
            add("Highcharts");
        }
        report.frontend = Some(frontend);
        Ok(())
    }
}

fn collect_python_deps_from_requirements(
    path: &Path,
    ctx: &DetectContext,
    out: &mut Vec<String>,
) -> Result<(), DetectError> {
    let mut visited = HashSet::new();
    collect_python_deps_from_requirements_inner(
        path,
        ctx,
        out,
        0,
        ctx.config.follow_requirements_depth,
        &mut visited,
    )
}

fn collect_python_deps_from_requirements_inner(
    path: &Path,
    ctx: &DetectContext,
    out: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), DetectError> {
    if depth > max_depth {
        return Ok(());
    }
    let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canon) {
        return Ok(());
    }

    let Some(raw) = (match read_text_with_limit(path, ctx.config.max_config_bytes) {
        Ok(v) => v,
        Err(e) => return Err(e),
    }) else {
        return Ok(());
    };

    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with('-') {
            if let Some(rest) = line.strip_prefix("-r").or_else(|| line.strip_prefix("--requirement")) {
                let f = rest.trim();
                if !f.is_empty() {
                    let included = base_dir.join(f);
                    let _ = collect_python_deps_from_requirements_inner(
                        &included,
                        ctx,
                        out,
                        depth + 1,
                        max_depth,
                        visited,
                    );
                }
            }
            continue;
        }

        let mut part = line;
        if let Some((left, _)) = part.split_once('#') {
            part = left.trim();
        }
        if part.is_empty() {
            continue;
        }
        if part.contains("git+") || part.contains("://") {
            continue;
        }

        let name = normalize_requirement_name(part);
        if !name.is_empty() {
            out.push(name);
        }
    }
    Ok(())
}

fn normalize_requirement_name(line: &str) -> String {
    let s = line.trim();
    let mut cut = s.len();
    for op in ["==", ">=", "<=", "!=", ">", "<", ";"] {
        if let Some(idx) = s.find(op) {
            cut = cut.min(idx);
        }
    }
    let mut name = s[..cut].trim();
    if let Some(idx) = name.find('[') {
        name = name[..idx].trim();
    }
    name.to_ascii_lowercase()
}

fn collect_python_deps_from_pyproject(
    path: &Path,
    ctx: &DetectContext,
    out: &mut Vec<String>,
) -> Result<(), DetectError> {
    let Some(raw) = (match read_text_with_limit(path, ctx.config.max_config_bytes) {
        Ok(v) => v,
        Err(e) => return Err(e),
    }) else {
        return Ok(());
    };

    let parsed: toml::Value = raw
        .parse::<toml::Value>()
        .map_err(|e| DetectError::Parse(format!("pyproject.toml 解析失败: {}", e)))?;

    let keys = extract_poetry_dependency_keys(&parsed);
    out.extend(keys);
    Ok(())
}

fn extract_poetry_dependency_keys(v: &toml::Value) -> Vec<String> {
    let mut out = Vec::new();
    let deps = v
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_table());
    if let Some(table) = deps {
        for (k, _) in table {
            out.push(k.to_ascii_lowercase());
        }
    }
    let dev_deps = v
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("group"))
        .and_then(|g| g.get("dev"))
        .and_then(|d| d.get("dependencies"))
        .and_then(|d| d.as_table());
    if let Some(table) = dev_deps {
        for (k, _) in table {
            out.push(k.to_ascii_lowercase());
        }
    }
    out
}

fn collect_python_deps_from_pipfile(
    path: &Path,
    ctx: &DetectContext,
    out: &mut Vec<String>,
) -> Result<(), DetectError> {
    let Some(raw) = (match read_text_with_limit(path, ctx.config.max_config_bytes) {
        Ok(v) => v,
        Err(e) => return Err(e),
    }) else {
        return Ok(());
    };

    let mut section = "";
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        if section != "packages" && section != "dev-packages" {
            continue;
        }
        if let Some((k, _)) = line.split_once('=') {
            let name = k.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                out.push(name.to_ascii_lowercase());
            }
        }
    }
    Ok(())
}

struct ViteDetector;

impl Detector for ViteDetector {
    fn id(&self) -> &'static str {
        "ViteDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Frontend {
            return Ok(());
        }
        let mut roots: Vec<&Path> = vec![&module.abs_path];
        if module
            .abs_path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("src"))
        {
            if let Some(parent) = module.abs_path.parent() {
                roots.push(parent);
            }
        }

        let mut config_path: Option<PathBuf> = None;
        for r in roots {
            let ts = r.join("vite.config.ts");
            let js = r.join("vite.config.js");
            if ts.exists() {
                config_path = Some(ts);
                break;
            }
            if js.exists() {
                config_path = Some(js);
                break;
            }
        }
        let Some(config_path) = config_path else {
            return Ok(());
        };
        if let Err(e) = read_text_with_limit(&config_path, ctx.config.max_config_bytes) {
            report
                .warnings
                .push(format!("vite.config: {}", describe_error(e)));
        }

        report.frameworks.push("Vite".to_string());
        let mut frontend = report.frontend.clone().unwrap_or(FrontendStack {
            builder: "Unknown".to_string(),
            vue: None,
            store: "None".to_string(),
            ui: "None".to_string(),
            visualization: Vec::new(),
        });
        frontend.builder = "Vite".to_string();
        report.frontend = Some(frontend);
        Ok(())
    }
}

struct PackageJsonDepsDetector;

impl Detector for PackageJsonDepsDetector {
    fn id(&self) -> &'static str {
        "PackageJsonDepsDetector"
    }

    fn detect(
        &self,
        ctx: &DetectContext,
        module: &ModuleContext,
        report: &mut ModuleReport,
    ) -> Result<(), DetectError> {
        if module.kind != ModuleKind::Frontend {
            return Ok(());
        }
        let mut package_json = module.abs_path.join("package.json");
        if !package_json.exists()
            && module
                .abs_path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|v| v.eq_ignore_ascii_case("src"))
        {
            if let Some(parent) = module.abs_path.parent() {
                let alt = parent.join("package.json");
                if alt.exists() {
                    package_json = alt;
                }
            }
        }

        let Some(raw) = (match read_text_with_limit(&package_json, ctx.config.max_config_bytes) {
            Ok(v) => v,
            Err(e) => {
                report.warnings.push(describe_error(e));
                return Ok(());
            }
        }) else {
            return Ok(());
        };
        let json: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                report.warnings.push(format!("package.json 解析失败: {}", e));
                return Ok(());
            }
        };
        let deps = json.get("dependencies").and_then(|d| d.as_object());
        if deps.is_none() {
            return Ok(());
        }
        let deps = deps.unwrap();

        let mut frontend = report.frontend.clone().unwrap_or(FrontendStack {
            builder: "Unknown".to_string(),
            vue: None,
            store: "None".to_string(),
            ui: "None".to_string(),
            visualization: Vec::new(),
        });

        if let Some(v) = deps.get("vue").and_then(|v| v.as_str()) {
            if v.trim().starts_with("^3") || v.trim().starts_with("3") || v.contains("3.") {
                frontend.vue = Some(3);
                report.frameworks.push("Vue3".to_string());
            }
        }
        if deps.contains_key("react") && deps.contains_key("react-dom") {
            report.frameworks.push("React".to_string());
        }
        if deps.contains_key("next") {
            report.frameworks.push("Next.js".to_string());
        }
        if deps.contains_key("svelte") {
            report.frameworks.push("Svelte".to_string());
        }
        if deps.contains_key("pinia") {
            frontend.store = "Pinia".to_string();
            report.deps.push("Pinia".to_string());
        }
        if frontend.store == "None" {
            if deps.contains_key("@reduxjs/toolkit") || deps.contains_key("redux") {
                frontend.store = "Redux".to_string();
                report.deps.push("Redux".to_string());
            } else if deps.contains_key("zustand") {
                frontend.store = "Zustand".to_string();
                report.deps.push("Zustand".to_string());
            } else if deps.contains_key("recoil") {
                frontend.store = "Recoil".to_string();
                report.deps.push("Recoil".to_string());
            }
        }
        if deps.contains_key("element-plus") {
            frontend.ui = "ElementPlus".to_string();
            report.deps.push("ElementPlus".to_string());
        } else if deps.contains_key("antd-vue") {
            frontend.ui = "AntdVue".to_string();
            report.deps.push("AntdVue".to_string());
        } else if deps.contains_key("vuetify") {
            frontend.ui = "Vuetify".to_string();
            report.deps.push("Vuetify".to_string());
        }
        if frontend.ui == "None" {
            if deps.contains_key("@mui/material") {
                frontend.ui = "MUI".to_string();
                report.deps.push("MUI".to_string());
            } else if deps.contains_key("antd") {
                frontend.ui = "Ant Design".to_string();
                report.deps.push("Ant Design".to_string());
            } else if deps.contains_key("tailwindcss") {
                frontend.ui = "Tailwind CSS".to_string();
                report.deps.push("Tailwind CSS".to_string());
            }
        } else if deps.contains_key("tailwindcss") {
            report.deps.push("Tailwind CSS".to_string());
        }
        if deps.contains_key("lucide-react") {
            report.deps.push("Lucide Icons".to_string());
        }
        report.frontend = Some(frontend);
        Ok(())
    }
}

#[tauri::command]
pub async fn scan_semantic_local(root_path: String) -> Result<SemanticReport, String> {
    let root_path = root_path.trim().to_string();
    if root_path.is_empty() {
        return Err("请输入项目根目录".to_string());
    }
    let root = PathBuf::from(&root_path);
    if !root.exists() {
        return Err("指定目录不存在".to_string());
    }
    if !root.is_dir() {
        return Err("指定路径不是目录".to_string());
    }
    tokio::task::spawn_blocking(move || scan_semantic_repo(&root, SemanticScanConfig::default()))
        .await
        .map_err(|e| format!("扫描任务失败: {}", e))?
}

#[tauri::command]
pub async fn scan_semantic_github(repo_url: String) -> Result<SemanticReport, String> {
    let repo_url = repo_url.trim().to_string();
    if repo_url.is_empty() {
        return Err("请输入 GitHub 仓库链接".to_string());
    }
    let parsed = parse_github_input(&repo_url)?;
    let zip_bytes = download_github_zip(&parsed).await?;
    let temp = tempfile::tempdir().map_err(|e| format!("创建临时目录失败: {}", e))?;
    let extracted_root = extract_zip_to_dir(&zip_bytes, temp.path())
        .map_err(|e| format!("解压失败: {}", e))?;
    let scan_root = if let Some(subdir) = &parsed.subdir {
        extracted_root.join(subdir)
    } else {
        extracted_root.clone()
    };
    if !scan_root.exists() || !scan_root.is_dir() {
        return Err("指定的子目录不存在或不是目录".to_string());
    }
    let display = parsed.display.clone();
    tokio::task::spawn_blocking(move || {
        let _keep = temp;
        let report = scan_semantic_repo(&scan_root, SemanticScanConfig::default())?;
        Ok(SemanticReport {
            repo_root: display,
            ..report
        })
    })
    .await
    .map_err(|e| format!("扫描任务失败: {}", e))?
}

#[tauri::command]
pub async fn export_semantic_json(path: String, report: SemanticReport) -> Result<(), String> {
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err("请选择导出路径".to_string());
    }
    if !path.to_lowercase().ends_with(".json") {
        return Err("仅支持导出为 .json 文件".to_string());
    }
    let content =
        serde_json::to_string_pretty(&report).map_err(|e| format!("序列化失败: {}", e))?;
    tokio::task::spawn_blocking(move || write_bytes_atomic(&path, content.as_bytes()))
        .await
        .map_err(|e| format!("导出任务失败: {}", e))?
}

#[tauri::command]
pub async fn export_semantic_schema() -> Result<String, String> {
    Ok(schema_as_json_string())
}

fn write_bytes_atomic(path: &str, bytes: &[u8]) -> Result<(), String> {
    let target = PathBuf::from(path);
    let parent = target
        .parent()
        .ok_or_else(|| "导出路径无效".to_string())?;
    if !parent.exists() {
        return Err("导出目录不存在".to_string());
    }
    let tmp = parent.join(format!(".semantic-export-{}.tmp", Utc::now().timestamp_millis()));
    {
        let mut file = fs::File::create(&tmp).map_err(|e| format!("写入失败: {}", e))?;
        file.write_all(bytes).map_err(|e| format!("写入失败: {}", e))?;
        let _ = file.sync_all();
    }
    fs::rename(&tmp, &target).map_err(|e| format!("保存失败: {}", e))?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ParsedGithubInput {
    owner: String,
    repo: String,
    reference: String,
    subdir: Option<String>,
    display: String,
}

fn parse_github_input(input: &str) -> Result<ParsedGithubInput, String> {
    let s = input.trim();
    let s = s
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_end_matches('/');
    let s = if let Some(rest) = s.strip_prefix("github.com/") {
        rest
    } else {
        s
    };
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return Err("GitHub 链接格式不正确".to_string());
    }
    let owner = parts[0].to_string();
    let repo = parts[1].to_string();

    let mut reference = "HEAD".to_string();
    let mut subdir: Option<String> = None;
    if parts.len() >= 4 && parts[2] == "tree" {
        reference = parts[3].to_string();
        if parts.len() > 4 {
            subdir = Some(parts[4..].join("/"));
        }
    }

    let display = if let Some(sd) = &subdir {
        format!("{}/{}@{}:{}", owner, repo, reference, sd)
    } else {
        format!("{}/{}@{}", owner, repo, reference)
    };
    Ok(ParsedGithubInput {
        owner,
        repo,
        reference,
        subdir,
        display,
    })
}

async fn download_github_zip(parsed: &ParsedGithubInput) -> Result<Vec<u8>, String> {
    let url = format!(
        "https://codeload.github.com/{}/{}/zip/{}",
        parsed.owner, parsed.repo, parsed.reference
    );
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败: {}", e))?;
    let res = client
        .get(&url)
        .header(USER_AGENT, "my-toolbox/semantic-scan")
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !res.status().is_success() {
        return Err(format!("下载失败，HTTP {}", res.status()));
    }
    res.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("读取下载内容失败: {}", e))
}

fn extract_zip_to_dir(zip_bytes: &[u8], target: &Path) -> Result<PathBuf, std::io::Error> {
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))?;
    let mut root_dir: Option<String> = None;
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        if let Some((first, _)) = name.split_once('/') {
            if !first.is_empty() {
                root_dir.get_or_insert_with(|| first.to_string());
            }
        }
    }
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name();
        let mut out_path = target.to_path_buf();
        for comp in name.split('/') {
            if comp.is_empty() {
                continue;
            }
            out_path.push(comp);
        }
        if file.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out_file = fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut out_file)?;
    }
    let extracted = if let Some(r) = root_dir {
        target.join(r)
    } else {
        target.to_path_buf()
    };
    Ok(extracted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn monorepo_backend_frontend_split() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("backend")).unwrap();
        fs::create_dir_all(root.join("frontend")).unwrap();

        write_file(root.join("backend/manage.py").as_path(), "print('ok')\n");
        write_file(
            root.join("backend/requirements.txt").as_path(),
            "Django>=3.2,<4.0\n# comment\ncelery\npsycopg2-binary\n",
        );
        write_file(root.join("frontend/vite.config.ts").as_path(), "export default {};\n");
        write_file(
            root.join("frontend/package.json").as_path(),
            r#"{ "dependencies": { "vue": "^3.4.0", "pinia": "^2.0.0", "element-plus": "^2.0.0" } }"#,
        );

        let report = scan_semantic_repo(root, SemanticScanConfig::default()).unwrap();
        assert_eq!(report.modules.len(), 2);
        assert!(report.modules.iter().any(|m| m.path == "/backend"));
        assert!(report.modules.iter().any(|m| m.path == "/frontend"));
    }

    #[test]
    fn missing_manage_or_requirements_falls_back_unknown() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("backend")).unwrap();
        write_file(root.join("backend/requirements.txt").as_path(), "celery\n");

        let report = scan_semantic_repo(root, SemanticScanConfig::default()).unwrap();
        let module = report.modules.iter().find(|m| m.path == "/backend").unwrap();
        let backend = module.backend.clone().unwrap();
        assert_eq!(backend.framework, "Python App (Generic)");
        assert_eq!(backend.queue, "Celery");
    }

    #[test]
    fn db_driver_priority_psycopg2_over_mysqlclient() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("backend")).unwrap();
        write_file(root.join("backend/manage.py").as_path(), "print('ok')\n");
        write_file(
            root.join("backend/requirements.txt").as_path(),
            "mysqlclient==2.1.0\npsycopg2>=2.9\n",
        );

        let report = scan_semantic_repo(root, SemanticScanConfig::default()).unwrap();
        let module = report.modules.iter().find(|m| m.path == "/backend").unwrap();
        let backend = module.backend.clone().unwrap();
        assert_eq!(backend.db, "PostgreSQL");
    }

    #[test]
    fn deep_nested_migrations_is_generated() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("backend")).unwrap();
        write_file(root.join("backend/manage.py").as_path(), "print('ok')\n");
        write_file(
            root.join("backend/apps/deep/nested/migrations/001.py").as_path(),
            "from django.db import migrations\n\nclass Migration(migrations.Migration):\n    pass\n",
        );

        let report = scan_semantic_repo(root, SemanticScanConfig::default()).unwrap();
        let module = report.modules.iter().find(|m| m.path == "/backend").unwrap();
        assert!(module.generated.is_some());
        assert!(report.summary.generated_files >= 1);
    }

    #[test]
    fn invalid_package_json_is_warning_not_crash() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("backend")).unwrap();
        fs::create_dir_all(root.join("frontend")).unwrap();
        write_file(root.join("backend/manage.py").as_path(), "print('ok')\n");
        write_file(root.join("frontend/vite.config.ts").as_path(), "export default {};\n");
        write_file(root.join("frontend/package.json").as_path(), "{ invalid json }");

        let report = scan_semantic_repo(root, SemanticScanConfig::default()).unwrap();
        let frontend = report
            .modules
            .iter()
            .find(|m| m.path == "/frontend")
            .unwrap();
        assert!(frontend.frameworks.iter().any(|v| v == "Vite"));
        assert!(frontend.warnings.iter().any(|w| w.contains("package.json")));
    }

    #[test]
    fn learning_analytics_system_like_detects_flask_sqlalchemy_alembic_ai() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("backend")).unwrap();
        write_file(root.join("backend/run.py").as_path(), "from app import create_app\n");
        write_file(
            root.join("backend/app/__init__.py").as_path(),
            "from flask import Flask\n\napp = Flask(__name__)\n",
        );
        write_file(root.join("backend/migrations/alembic.ini").as_path(), "[alembic]\n");
        write_file(
            root.join("backend/app/services/ai_planner/llm_client.py").as_path(),
            "import openai\n",
        );
        write_file(
            root.join("backend/requirements.txt").as_path(),
            "flask\nsqlalchemy\nalembic\nopenai\n",
        );

        let report = scan_semantic_repo(root, SemanticScanConfig::default()).unwrap();
        let backend = report.modules.iter().find(|m| m.path == "/backend").unwrap();
        let stack = backend.backend.clone().unwrap();
        assert_eq!(stack.framework, "Flask");
        assert_eq!(stack.orm.as_deref(), Some("SQLAlchemy"));
        assert_eq!(stack.migrations.as_deref(), Some("Alembic"));
        assert!(stack.ai_features.iter().any(|v| v == "OpenAI"));
        assert!(stack.ai_features.iter().any(|v| v == "Custom AI Logic"));
    }

    #[test]
    fn tauri_project_like_detects_rust_tauri_and_react_tailwind_vite() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src-tauri").join("src")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();

        write_file(
            root.join("src-tauri/Cargo.toml").as_path(),
            r#"
[package]
name = "demo"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = "2"
rusqlite = { version = "0.30" }
serde = "1"
"#,
        );
        write_file(root.join("src-tauri/tauri.conf.json").as_path(), r#"{}"#);
        write_file(
            root.join("src-tauri/src/main.rs").as_path(),
            "fn main() { println!(\"ok\"); }\n",
        );

        write_file(
            root.join("package.json").as_path(),
            r#"{ "dependencies": { "react": "^18.0.0", "react-dom": "^18.0.0", "tailwindcss": "^3.0.0" } }"#,
        );
        write_file(root.join("vite.config.ts").as_path(), "export default {};\n");
        write_file(root.join("src/main.tsx").as_path(), "export const App = () => null;\n");

        let report = scan_semantic_repo(root, SemanticScanConfig::default()).unwrap();
        let backend = report
            .modules
            .iter()
            .find(|m| m.path == "/src-tauri")
            .unwrap();
        assert_eq!(backend.name, "Tauri Core");
        let stack = backend.backend.clone().unwrap();
        assert_eq!(stack.framework, "Tauri");
        assert_eq!(stack.db, "SQLite");
        assert!(backend.frameworks.iter().any(|v| v == "Tauri"));
        assert!(backend.deps.iter().any(|v| v == "SQLite"));
        assert!(backend.languages.iter().any(|l| l.language == "Rust"));

        let frontend = report.modules.iter().find(|m| m.path == "/src").unwrap();
        let fe = frontend.frontend.clone().unwrap();
        assert_eq!(fe.builder, "Vite");
        assert!(frontend.frameworks.iter().any(|v| v == "React"));
        assert!(frontend.deps.iter().any(|v| v == "Tailwind CSS"));
        assert_eq!(fe.ui, "Tailwind CSS");
        assert!(frontend.languages.iter().any(|l| l.language == "TypeScript"));
    }

    #[test]
    fn detects_flutter_project() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("lib")).unwrap();
        fs::create_dir_all(root.join("android")).unwrap();

        write_file(
            root.join("pubspec.yaml").as_path(),
            r#"
name: demo
environment:
  sdk: ">=3.0.0 <4.0.0"
dependencies:
  flutter:
    sdk: flutter
  isar: ^3.1.0
  flutter_riverpod: ^2.0.0
"#,
        );
        write_file(root.join("lib/main.dart").as_path(), "void main() {}\n");
        write_file(
            root.join("android/build.gradle").as_path(),
            "android { defaultConfig { minSdkVersion 24 targetSdkVersion 34 } }\n",
        );

        let report = scan_semantic_repo(root, SemanticScanConfig::default()).unwrap();
        let module = report.modules.iter().find(|m| m.path == "/").unwrap();
        assert_eq!(module.name, "Flutter App");
        assert!(module.frameworks.iter().any(|v| v == "Flutter"));
        assert!(module.languages.iter().any(|l| l.language == "Dart"));
        assert!(module.deps.iter().any(|v| v == "Isar"));
        assert!(module.deps.iter().any(|v| v == "Riverpod"));
        assert!(module.deps.iter().any(|v| v == "Android"));
        assert!(
            module
                .deps
                .iter()
                .any(|v| v == "Dart SDK >=3.0.0 <4.0.0")
        );
    }
}
