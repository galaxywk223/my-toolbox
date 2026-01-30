use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Command,
    time::UNIX_EPOCH,
    time::Instant,
};
use tauri::Emitter;
use ignore::WalkBuilder;
use printpdf::{BuiltinFont, Mm, PdfDocument, PdfDocumentReference};
use zip::ZipArchive;
use crate::db::{resolve_db_path, Database};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechStackScanOptions {
    pub max_files: usize,
    pub max_total_bytes: u64,
}

impl Default for TechStackScanOptions {
    fn default() -> Self {
        Self {
            max_files: 6000,
            max_total_bytes: 40 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechStackProgress {
    pub stage: String,
    pub detail: String,
    pub processed_files: usize,
    pub total_files_hint: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechComponent {
    pub id: String,
    pub name: String,
    pub category: String,
    pub version: Option<String>,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStat {
    pub language: String,
    pub bytes: u64,
    pub files: usize,
    pub percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechGraphNode {
    pub id: String,
    pub label: String,
    pub category: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechGraphEdge {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechGraph {
    pub nodes: Vec<TechGraphNode>,
    pub edges: Vec<TechGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechStackReport {
    pub input_kind: String,
    pub input_value: String,
    pub root_display: String,
    pub elapsed_ms: u128,
    pub detected: Vec<TechComponent>,
    pub languages: Vec<LanguageStat>,
    pub package_managers: Vec<String>,
    pub build_tools: Vec<String>,
    pub test_frameworks: Vec<String>,
    pub submodules: Vec<GitSubmodule>,
    pub graph: TechGraph,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSubmodule {
    pub name: String,
    pub path: String,
    pub url: Option<String>,
    pub scanned: bool,
    pub scan_warning: Option<String>,
}

#[tauri::command]
pub async fn scan_tech_stack_local(
    root_path: String,
    options: Option<TechStackScanOptions>,
    window: tauri::Window,
) -> Result<TechStackReport, String> {
    let root_path = root_path.trim().to_string();
    if root_path.is_empty() {
        return Err("请输入项目根目录路径".to_string());
    }

    let root = PathBuf::from(&root_path);
    if !root.exists() {
        return Err("指定目录不存在".to_string());
    }
    if !root.is_dir() {
        return Err("指定路径不是目录".to_string());
    }

    let options = options.unwrap_or_default();
    let root_display = root.to_string_lossy().to_string();
    tokio::task::spawn_blocking(move || scan_local_blocking(root, root_display, options, window))
        .await
        .map_err(|e| format!("扫描任务失败: {}", e))?
}

#[tauri::command]
pub async fn scan_tech_stack_github(
    repo_url: String,
    options: Option<TechStackScanOptions>,
    window: tauri::Window,
) -> Result<TechStackReport, String> {
    let repo_url = repo_url.trim().to_string();
    if repo_url.is_empty() {
        return Err("请输入 GitHub 仓库链接".to_string());
    }
    let parsed = parse_github_input(&repo_url)?;
    emit_progress(
        Some(&window),
        "下载仓库",
        &format!("准备下载 {} / {}", parsed.owner, parsed.repo),
        0,
        None,
    );

    let zip_bytes = download_github_zip(&parsed, &window).await?;
    emit_progress(Some(&window), "解压仓库", "正在解压到临时目录", 0, None);

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

    let root_display = parsed.display.clone();
    let options = options.unwrap_or_default();
    let original = repo_url.clone();
    tokio::task::spawn_blocking(move || {
        let _temp = temp;
        scan_dir_blocking(
            "github",
            &original,
            scan_root,
            root_display,
            options,
            window,
        )
    })
    .await
    .map_err(|e| format!("扫描任务失败: {}", e))?
}

#[tauri::command]
pub async fn export_tech_stack_json(path: String, report: TechStackReport) -> Result<(), String> {
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
pub async fn export_tech_stack_pdf(path: String, report: TechStackReport) -> Result<(), String> {
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err("请选择导出路径".to_string());
    }
    if !path.to_lowercase().ends_with(".pdf") {
        return Err("仅支持导出为 .pdf 文件".to_string());
    }
    tokio::task::spawn_blocking(move || {
        let bytes = build_pdf_bytes(&report)?;
        write_bytes_atomic(&path, &bytes)
    })
    .await
    .map_err(|e| format!("导出任务失败: {}", e))?
}

fn scan_local_blocking(
    root: PathBuf,
    root_display: String,
    options: TechStackScanOptions,
    window: tauri::Window,
) -> Result<TechStackReport, String> {
    let input_value = root_display.clone();
    scan_dir_blocking("local", &input_value, root, root_display, options, window)
}

fn scan_dir_blocking(
    input_kind: &str,
    input_value: &str,
    root: PathBuf,
    root_display: String,
    options: TechStackScanOptions,
    window: tauri::Window,
) -> Result<TechStackReport, String> {
    let start = Instant::now();
    emit_progress(
        Some(&window),
        "读取配置",
        "开始读取关键配置文件",
        0,
        None,
    );

    let fingerprint = build_scan_fingerprint(input_kind, input_value, &root);
    if let Some(db) = open_cache_db() {
        if let Ok(Some(cached_json)) = db.get_tech_stack_scan_json(input_kind, &fingerprint) {
            if let Ok(mut report) = serde_json::from_str::<TechStackReport>(&cached_json) {
                report.warnings.push("缓存命中".to_string());
                emit_progress(Some(&window), "缓存命中", "已直接返回缓存结果", 0, None);
                return Ok(report);
            }
        }
    }

    let mut warnings: Vec<String> = Vec::new();
    let mut detected: Vec<TechComponent> = Vec::new();
    let mut graph = TechGraph {
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    let mut submodules = parse_git_submodules(&root);
    let excluded_dirs: Vec<PathBuf> = submodules.iter().map(|m| root.join(&m.path)).collect();

    let package_json_path = root.join("package.json");
    if package_json_path.exists() {
        match fs::read_to_string(&package_json_path) {
            Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                Ok(json) => analyze_package_json(&json, &mut detected, &mut graph),
                Err(_) => warnings.push("package.json 解析失败".to_string()),
            },
            Err(_) => warnings.push("package.json 读取失败".to_string()),
        }
    }

    analyze_non_js_configs(&root, &mut detected, &mut warnings);

    for m in &mut submodules {
        let sub_root = root.join(&m.path);
        if !sub_root.exists() || !sub_root.is_dir() {
            m.scanned = false;
            m.scan_warning = Some("子模块目录不存在或不可访问".to_string());
            continue;
        }

        let mut sub_warnings: Vec<String> = Vec::new();
        let mut sub_detected: Vec<TechComponent> = Vec::new();
        let mut sub_graph = TechGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        let sub_package_json = sub_root.join("package.json");
        if sub_package_json.exists() {
            if let Ok(raw) = fs::read_to_string(&sub_package_json) {
                if let Ok(json) = serde_json::from_str::<Value>(&raw) {
                    analyze_package_json(&json, &mut sub_detected, &mut sub_graph);
                }
            }
        }
        analyze_non_js_configs(&sub_root, &mut sub_detected, &mut sub_warnings);

        for c in sub_detected {
            detected.push(TechComponent {
                evidence: c
                    .evidence
                    .into_iter()
                    .map(|e| format!("子模块 {}: {}", m.path, e))
                    .collect(),
                ..c
            });
        }

        for node in sub_graph.nodes {
            graph.nodes.push(TechGraphNode {
                id: format!("{}::{}", m.path, node.id),
                label: node.label,
                category: node.category,
                version: node.version,
            });
        }
        for edge in sub_graph.edges {
            graph.edges.push(TechGraphEdge {
                from: format!("{}::{}", m.path, edge.from),
                to: format!("{}::{}", m.path, edge.to),
                label: edge.label,
            });
        }

        m.scanned = true;
        if !sub_warnings.is_empty() {
            m.scan_warning = Some(sub_warnings.join("；"));
        }
    }

    let mut package_managers = detect_package_managers(&root);
    package_managers.sort();
    package_managers.dedup();

    let mut build_tools = detect_build_tools(&detected);
    build_tools.sort();
    build_tools.dedup();

    let mut test_frameworks = detect_test_frameworks(&detected);
    test_frameworks.sort();
    test_frameworks.dedup();

    emit_progress(
        Some(&window),
        "统计语言",
        "进行文件扩展名采样统计",
        0,
        None,
    );

    let (mut language_bytes, mut language_files, processed_files, mut stopped_early) =
        collect_language_stats(&root, &options, Some(&window), &excluded_dirs);

    for m in &submodules {
        if !m.scanned {
            continue;
        }
        let sub_root = root.join(&m.path);
        let (sub_bytes, sub_files, _processed, sub_stopped) =
            collect_language_stats(&sub_root, &options, Some(&window), &[]);
        stopped_early = stopped_early || sub_stopped;
        for (k, v) in sub_bytes {
            *language_bytes.entry(k).or_insert(0) += v;
        }
        for (k, v) in sub_files {
            *language_files.entry(k).or_insert(0) += v;
        }
    }
    if stopped_early {
        warnings.push("项目较大，已按阈值进行采样统计（可能存在少量漏判）".to_string());
    }

    let languages = compute_language_percentages(language_bytes, language_files);

    emit_progress(
        Some(&window),
        "汇总报告",
        "生成技术栈报告与关系图",
        processed_files,
        None,
    );
    normalize_components(&mut detected);

    let report = TechStackReport {
        input_kind: input_kind.to_string(),
        input_value: input_value.to_string(),
        root_display,
        elapsed_ms: start.elapsed().as_millis(),
        detected,
        languages,
        package_managers,
        build_tools,
        test_frameworks,
        submodules,
        graph,
        warnings,
    };

    if let Some(db) = open_cache_db() {
        if let Ok(json) = serde_json::to_string(&report) {
            let elapsed = report.elapsed_ms.min(i64::MAX as u128) as i64;
            let _ = db.upsert_tech_stack_scan_json(
                input_kind,
                input_value,
                &fingerprint,
                &json,
                elapsed,
            );
        }
    }

    Ok(report)
}

fn write_bytes_atomic(path: &str, bytes: &[u8]) -> Result<(), String> {
    let target = PathBuf::from(path);
    let parent = target
        .parent()
        .ok_or_else(|| "导出路径无效".to_string())?;
    if !parent.exists() {
        return Err("导出目录不存在".to_string());
    }

    let tmp_name = format!(
        ".tech-stack-export-{}.tmp",
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let tmp = parent.join(tmp_name);
    {
        let mut file = fs::File::create(&tmp).map_err(|e| format!("写入失败: {}", e))?;
        file.write_all(bytes).map_err(|e| format!("写入失败: {}", e))?;
        let _ = file.sync_all();
    }
    fs::rename(&tmp, &target).map_err(|e| format!("保存失败: {}", e))?;
    Ok(())
}

fn build_pdf_bytes(report: &TechStackReport) -> Result<Vec<u8>, String> {
    let (doc, page1, layer1) = PdfDocument::new("Tech Stack Report", Mm(210.0), Mm(297.0), "L1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| format!("PDF 字体初始化失败: {}", e))?;

    let mut pages: Vec<(printpdf::PdfPageIndex, printpdf::PdfLayerIndex)> = vec![(page1, layer1)];
    let mut current = 0usize;
    let mut y_mm: f32 = 285.0;

    let mut lines = Vec::new();
    lines.push("技术栈报告".to_string());
    lines.push(format!("输入类型：{}", report.input_kind));
    lines.push(format!("输入：{}", report.input_value));
    lines.push(format!("根目录：{}", report.root_display));
    lines.push(format!("耗时：{} ms", report.elapsed_ms));
    if !report.package_managers.is_empty() {
        lines.push(format!("包管理器：{}", report.package_managers.join(" / ")));
    }
    if !report.build_tools.is_empty() {
        lines.push(format!("构建工具：{}", report.build_tools.join(" / ")));
    }
    if !report.test_frameworks.is_empty() {
        lines.push(format!("测试框架：{}", report.test_frameworks.join(" / ")));
    }
    if !report.warnings.is_empty() {
        lines.push("提示：".to_string());
        for w in &report.warnings {
            lines.push(format!("- {}", w));
        }
    }
    lines.push("".to_string());
    lines.push("语言占比：".to_string());
    for l in report.languages.iter().take(12) {
        lines.push(format!(
            "- {}: {:.1}%（{} 文件，{}）",
            l.language,
            l.percent,
            l.files,
            l.bytes
        ));
    }
    if !report.submodules.is_empty() {
        lines.push("".to_string());
        lines.push("子模块：".to_string());
        for m in &report.submodules {
            let status = if m.scanned { "已扫描" } else { "未扫描" };
            let url = m.url.clone().unwrap_or_default();
            lines.push(format!("- {} ({}) {}", m.path, status, url));
        }
    }
    lines.push("".to_string());
    lines.push("检测到的技术组件：".to_string());
    for c in &report.detected {
        let ver = c.version.clone().unwrap_or_else(|| "unknown".to_string());
        let conf = (c.confidence * 100.0).round() as i32;
        lines.push(format!("- [{}] {} {} ({}%)", c.category, c.name, ver, conf));
    }

    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            write_pdf_line(&doc, &font, pages[current], 18.0, 20.0, y_mm, line);
            y_mm -= 12.0;
            continue;
        }
        if y_mm < 18.0 {
            let (p, l) = doc.add_page(Mm(210.0), Mm(297.0), format!("L{}", pages.len() + 1));
            pages.push((p, l));
            current = pages.len() - 1;
            y_mm = 285.0;
        }
        write_pdf_line(&doc, &font, pages[current], 10.5, 20.0, y_mm, line);
        y_mm -= 6.0;
    }

    let writer = Cursor::new(Vec::<u8>::new());
    let mut buf = std::io::BufWriter::new(writer);
    doc.save(&mut buf)
        .map_err(|e| format!("PDF 生成失败: {}", e))?;
    let cursor = buf
        .into_inner()
        .map_err(|e| format!("PDF 写入失败: {}", e))?;
    Ok(cursor.into_inner())
}

fn write_pdf_line(
    doc: &PdfDocumentReference,
    font: &printpdf::IndirectFontRef,
    page: (printpdf::PdfPageIndex, printpdf::PdfLayerIndex),
    size: f32,
    x: f32,
    y: f32,
    text: &str,
) {
    let layer = doc.get_page(page.0).get_layer(page.1);
    layer.use_text(text, size, Mm(x), Mm(y), font);
}

fn emit_progress(
    window: Option<&tauri::Window>,
    stage: &str,
    detail: &str,
    processed_files: usize,
    total_files_hint: Option<usize>,
) {
    let payload = TechStackProgress {
        stage: stage.to_string(),
        detail: detail.to_string(),
        processed_files,
        total_files_hint,
    };
    if let Some(window) = window {
        let _ = window.emit("tech_stack_progress", &payload);
    }
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
    if s.is_empty() {
        return Err("请输入 GitHub 仓库链接".to_string());
    }

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
        return Err("GitHub 链接格式不正确，应为 owner/repo".to_string());
    }
    let owner = parts[0].to_string();
    let repo = parts[1].trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err("GitHub 链接格式不正确，应为 owner/repo".to_string());
    }

    let mut reference = "HEAD".to_string();
    let mut subdir: Option<String> = None;
    if parts.len() >= 4 && parts[2] == "tree" {
        reference = parts[3].to_string();
        if parts.len() > 4 {
            subdir = Some(parts[4..].join("/"));
        }
    }
    if reference.trim().is_empty() {
        reference = "HEAD".to_string();
    }

    let display = match &subdir {
        Some(d) => format!("{}/{}@{}/{}", owner, repo, reference, d),
        None => format!("{}/{}@{}", owner, repo, reference),
    };

    Ok(ParsedGithubInput {
        owner,
        repo,
        reference,
        subdir,
        display,
    })
}

async fn download_github_zip(
    parsed: &ParsedGithubInput,
    window: &tauri::Window,
) -> Result<Vec<u8>, String> {
    let url = format!(
        "https://codeload.github.com/{}/{}/zip/{}",
        parsed.owner, parsed.repo, parsed.reference
    );
    let client = reqwest::Client::builder()
        .user_agent("my-toolbox/tech-stack-scanner")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("创建网络客户端失败: {}", e))?;

    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status().as_u16()));
    }

    let content_len = resp.content_length();
    let limit: u64 = 80 * 1024 * 1024;
    if let Some(len) = content_len {
        if len > limit {
            return Err("仓库压缩包过大，已拒绝下载".to_string());
        }
    }

    let mut buf: Vec<u8> = Vec::new();
    let mut downloaded: u64 = 0;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("下载失败: {}", e))?
    {
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > limit {
            return Err("仓库压缩包过大，已中止下载".to_string());
        }
        buf.extend_from_slice(&chunk);
        if downloaded % (2 * 1024 * 1024) < 65536 {
            let detail = match content_len {
                Some(total) if total > 0 => {
                    let percent = (downloaded as f64) * 100.0 / (total as f64);
                    format!("已下载 {:.1}%（{} MB）", percent, downloaded / (1024 * 1024))
                }
                _ => format!("已下载 {} MB", downloaded / (1024 * 1024)),
            };
            emit_progress(Some(window), "下载仓库", &detail, 0, None);
        }
    }
    Ok(buf)
}

fn extract_zip_to_dir(zip_bytes: &[u8], dest: &Path) -> Result<PathBuf, String> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("无法读取 zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("读取 zip 条目失败: {}", e))?;
        let name = file.name().to_string();
        let out_rel = sanitize_zip_path(&name)?;
        let out_path = dest.join(out_rel);

        if file.is_dir() {
            let _ = fs::create_dir_all(&out_path);
            continue;
        }

        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut out = fs::File::create(&out_path)
            .map_err(|e| format!("写入解压文件失败: {}: {}", out_path.to_string_lossy(), e))?;
        std::io::copy(&mut file, &mut out)
            .map_err(|e| format!("写入解压文件失败: {}: {}", out_path.to_string_lossy(), e))?;
        let _ = out.flush();
    }

    let mut top_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(dest) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                top_dirs.push(p);
            }
        }
    }
    if top_dirs.len() == 1 {
        Ok(top_dirs.remove(0))
    } else {
        Ok(dest.to_path_buf())
    }
}

fn sanitize_zip_path(name: &str) -> Result<PathBuf, String> {
    let p = Path::new(name);
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::Normal(s) => out.push(s),
            std::path::Component::CurDir => {}
            _ => return Err("zip 条目路径不安全，已拒绝解压".to_string()),
        }
    }
    Ok(out)
}

fn normalize_components(list: &mut Vec<TechComponent>) {
    let mut map: BTreeMap<String, TechComponent> = BTreeMap::new();
    for item in list.drain(..) {
        let key = format!("{}::{}", item.category, item.id);
        map.entry(key)
            .and_modify(|existing| {
                if item.confidence > existing.confidence {
                    existing.confidence = item.confidence;
                }
                if existing.version.is_none() {
                    existing.version = item.version.clone();
                }
                existing.evidence.extend(item.evidence.clone());
            })
            .or_insert(item);
    }
    let mut out: Vec<TechComponent> = map.into_values().collect();
    out.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    for item in &mut out {
        item.evidence.sort();
        item.evidence.dedup();
    }
    *list = out;
}

fn detect_package_managers(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if root.join("pnpm-lock.yaml").exists() {
        out.push("pnpm".to_string());
    }
    if root.join("yarn.lock").exists() {
        out.push("yarn".to_string());
    }
    if root.join("package-lock.json").exists() {
        out.push("npm".to_string());
    }
    if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        out.push("bun".to_string());
    }
    if root.join("poetry.lock").exists() {
        out.push("poetry".to_string());
    }
    if root.join("Pipfile.lock").exists() || root.join("Pipfile").exists() {
        out.push("pipenv".to_string());
    }
    if root.join("Gemfile.lock").exists() || root.join("Gemfile").exists() {
        out.push("bundler".to_string());
    }
    out
}

fn detect_build_tools(detected: &[TechComponent]) -> Vec<String> {
    detected
        .iter()
        .filter(|c| c.category == "build")
        .map(|c| c.name.clone())
        .collect()
}

fn detect_test_frameworks(detected: &[TechComponent]) -> Vec<String> {
    detected
        .iter()
        .filter(|c| c.category == "test")
        .map(|c| c.name.clone())
        .collect()
}

fn analyze_package_json(json: &Value, detected: &mut Vec<TechComponent>, graph: &mut TechGraph) {
    let deps = merge_dependency_objects(json);
    let (scripts, engines) = (
        json.get("scripts").cloned().unwrap_or(Value::Null),
        json.get("engines").cloned().unwrap_or(Value::Null),
    );

    if let Some(node_ver) = engines.get("node").and_then(|v| v.as_str()) {
        detected.push(TechComponent {
            id: "node".to_string(),
            name: "Node.js".to_string(),
            category: "runtime".to_string(),
            version: Some(node_ver.to_string()),
            confidence: 0.8,
            evidence: vec!["package.json: engines.node".to_string()],
        });
    }

    detect_js_frameworks(&deps, detected);
    detect_js_build_tools(&deps, &scripts, detected);
    detect_js_test_tools(&deps, &scripts, detected);
    detect_js_backend(&deps, detected);
    detect_js_db_tools(&deps, detected);

    build_js_dependency_graph(&deps, detected, graph);
}

fn merge_dependency_objects(json: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for key in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(obj) = json.get(key).and_then(|v| v.as_object()) {
            for (name, ver) in obj {
                if let Some(s) = ver.as_str() {
                    out.entry(name.to_string()).or_insert_with(|| s.to_string());
                }
            }
        }
    }
    out
}

fn detect_js_frameworks(deps: &BTreeMap<String, String>, detected: &mut Vec<TechComponent>) {
    let has = |name: &str| deps.contains_key(name);
    if has("react") {
        detected.push(TechComponent {
            id: "react".to_string(),
            name: "React".to_string(),
            category: "frontend".to_string(),
            version: deps.get("react").cloned(),
            confidence: 0.98,
            evidence: vec!["package.json: dependencies.react".to_string()],
        });
    }
    if has("next") {
        detected.push(TechComponent {
            id: "next".to_string(),
            name: "Next.js".to_string(),
            category: "frontend".to_string(),
            version: deps.get("next").cloned(),
            confidence: 0.95,
            evidence: vec!["package.json: dependencies.next".to_string()],
        });
    }
    if has("vue") {
        detected.push(TechComponent {
            id: "vue".to_string(),
            name: "Vue".to_string(),
            category: "frontend".to_string(),
            version: deps.get("vue").cloned(),
            confidence: 0.98,
            evidence: vec!["package.json: dependencies.vue".to_string()],
        });
    }
    if has("nuxt") || has("nuxt3") {
        let v = deps.get("nuxt").cloned().or_else(|| deps.get("nuxt3").cloned());
        detected.push(TechComponent {
            id: "nuxt".to_string(),
            name: "Nuxt".to_string(),
            category: "frontend".to_string(),
            version: v,
            confidence: 0.92,
            evidence: vec!["package.json: dependencies.nuxt".to_string()],
        });
    }
    if has("@angular/core") {
        detected.push(TechComponent {
            id: "angular".to_string(),
            name: "Angular".to_string(),
            category: "frontend".to_string(),
            version: deps.get("@angular/core").cloned(),
            confidence: 0.98,
            evidence: vec!["package.json: dependencies.@angular/core".to_string()],
        });
    }
    if has("svelte") {
        detected.push(TechComponent {
            id: "svelte".to_string(),
            name: "Svelte".to_string(),
            category: "frontend".to_string(),
            version: deps.get("svelte").cloned(),
            confidence: 0.97,
            evidence: vec!["package.json: dependencies.svelte".to_string()],
        });
    }
}

fn detect_js_build_tools(deps: &BTreeMap<String, String>, scripts: &Value, detected: &mut Vec<TechComponent>) {
    let has = |name: &str| deps.contains_key(name);
    if has("vite") || script_contains(scripts, "vite") {
        detected.push(TechComponent {
            id: "vite".to_string(),
            name: "Vite".to_string(),
            category: "build".to_string(),
            version: deps.get("vite").cloned(),
            confidence: 0.9,
            evidence: vec!["package.json: vite".to_string()],
        });
    }
    if has("webpack") || has("webpack-cli") || script_contains(scripts, "webpack") {
        detected.push(TechComponent {
            id: "webpack".to_string(),
            name: "Webpack".to_string(),
            category: "build".to_string(),
            version: deps.get("webpack").cloned().or_else(|| deps.get("webpack-cli").cloned()),
            confidence: 0.88,
            evidence: vec!["package.json: webpack".to_string()],
        });
    }
    if has("rollup") || script_contains(scripts, "rollup") {
        detected.push(TechComponent {
            id: "rollup".to_string(),
            name: "Rollup".to_string(),
            category: "build".to_string(),
            version: deps.get("rollup").cloned(),
            confidence: 0.85,
            evidence: vec!["package.json: rollup".to_string()],
        });
    }
}

fn detect_js_test_tools(deps: &BTreeMap<String, String>, scripts: &Value, detected: &mut Vec<TechComponent>) {
    let has = |name: &str| deps.contains_key(name);
    if has("vitest") || script_contains(scripts, "vitest") {
        detected.push(TechComponent {
            id: "vitest".to_string(),
            name: "Vitest".to_string(),
            category: "test".to_string(),
            version: deps.get("vitest").cloned(),
            confidence: 0.9,
            evidence: vec!["package.json: vitest".to_string()],
        });
    }
    if has("jest") || script_contains(scripts, "jest") {
        detected.push(TechComponent {
            id: "jest".to_string(),
            name: "Jest".to_string(),
            category: "test".to_string(),
            version: deps.get("jest").cloned(),
            confidence: 0.9,
            evidence: vec!["package.json: jest".to_string()],
        });
    }
    if has("cypress") {
        detected.push(TechComponent {
            id: "cypress".to_string(),
            name: "Cypress".to_string(),
            category: "test".to_string(),
            version: deps.get("cypress").cloned(),
            confidence: 0.9,
            evidence: vec!["package.json: cypress".to_string()],
        });
    }
    if has("@playwright/test") || has("playwright") {
        detected.push(TechComponent {
            id: "playwright".to_string(),
            name: "Playwright".to_string(),
            category: "test".to_string(),
            version: deps
                .get("@playwright/test")
                .cloned()
                .or_else(|| deps.get("playwright").cloned()),
            confidence: 0.88,
            evidence: vec!["package.json: playwright".to_string()],
        });
    }
}

fn detect_js_backend(deps: &BTreeMap<String, String>, detected: &mut Vec<TechComponent>) {
    let has = |name: &str| deps.contains_key(name);
    if has("express") {
        detected.push(TechComponent {
            id: "express".to_string(),
            name: "Express".to_string(),
            category: "backend".to_string(),
            version: deps.get("express").cloned(),
            confidence: 0.9,
            evidence: vec!["package.json: express".to_string()],
        });
    }
    if has("@nestjs/core") {
        detected.push(TechComponent {
            id: "nestjs".to_string(),
            name: "NestJS".to_string(),
            category: "backend".to_string(),
            version: deps.get("@nestjs/core").cloned(),
            confidence: 0.9,
            evidence: vec!["package.json: @nestjs/core".to_string()],
        });
    }
    if has("koa") {
        detected.push(TechComponent {
            id: "koa".to_string(),
            name: "Koa".to_string(),
            category: "backend".to_string(),
            version: deps.get("koa").cloned(),
            confidence: 0.85,
            evidence: vec!["package.json: koa".to_string()],
        });
    }
    if has("fastify") {
        detected.push(TechComponent {
            id: "fastify".to_string(),
            name: "Fastify".to_string(),
            category: "backend".to_string(),
            version: deps.get("fastify").cloned(),
            confidence: 0.88,
            evidence: vec!["package.json: fastify".to_string()],
        });
    }
}

fn detect_js_db_tools(deps: &BTreeMap<String, String>, detected: &mut Vec<TechComponent>) {
    let has = |name: &str| deps.contains_key(name);
    if has("prisma") {
        detected.push(TechComponent {
            id: "prisma".to_string(),
            name: "Prisma".to_string(),
            category: "database".to_string(),
            version: deps.get("prisma").cloned(),
            confidence: 0.85,
            evidence: vec!["package.json: prisma".to_string()],
        });
    }
    if has("mongoose") {
        detected.push(TechComponent {
            id: "mongoose".to_string(),
            name: "MongoDB (Mongoose)".to_string(),
            category: "database".to_string(),
            version: deps.get("mongoose").cloned(),
            confidence: 0.85,
            evidence: vec!["package.json: mongoose".to_string()],
        });
    }
    if has("pg") {
        detected.push(TechComponent {
            id: "postgres".to_string(),
            name: "PostgreSQL".to_string(),
            category: "database".to_string(),
            version: deps.get("pg").cloned(),
            confidence: 0.75,
            evidence: vec!["package.json: pg".to_string()],
        });
    }
    if has("mysql2") || has("mysql") {
        detected.push(TechComponent {
            id: "mysql".to_string(),
            name: "MySQL".to_string(),
            category: "database".to_string(),
            version: deps.get("mysql2").cloned().or_else(|| deps.get("mysql").cloned()),
            confidence: 0.7,
            evidence: vec!["package.json: mysql/mysql2".to_string()],
        });
    }
    if has("redis") || has("ioredis") {
        detected.push(TechComponent {
            id: "redis".to_string(),
            name: "Redis".to_string(),
            category: "database".to_string(),
            version: deps.get("redis").cloned().or_else(|| deps.get("ioredis").cloned()),
            confidence: 0.7,
            evidence: vec!["package.json: redis/ioredis".to_string()],
        });
    }
}

fn script_contains(scripts: &Value, needle: &str) -> bool {
    let Some(obj) = scripts.as_object() else {
        return false;
    };
    for v in obj.values() {
        if let Some(s) = v.as_str() {
            if s.contains(needle) {
                return true;
            }
        }
    }
    false
}

fn build_js_dependency_graph(
    deps: &BTreeMap<String, String>,
    detected: &[TechComponent],
    graph: &mut TechGraph,
) {
    let mut nodes: BTreeMap<String, TechGraphNode> = BTreeMap::new();
    for c in detected {
        nodes.entry(c.id.clone()).or_insert(TechGraphNode {
            id: c.id.clone(),
            label: c.name.clone(),
            category: c.category.clone(),
            version: c.version.clone(),
        });
    }
    let root_id = "package".to_string();
    nodes.entry(root_id.clone()).or_insert(TechGraphNode {
        id: root_id.clone(),
        label: "package.json".to_string(),
        category: "meta".to_string(),
        version: None,
    });

    let mut edges = Vec::new();
    for (name, ver) in deps {
        if nodes.contains_key(name) {
            edges.push(TechGraphEdge {
                from: root_id.clone(),
                to: name.to_string(),
                label: ver.to_string(),
            });
        }
    }

    graph.nodes = nodes.into_values().collect();
    graph.edges = edges;
}

fn analyze_non_js_configs(root: &Path, detected: &mut Vec<TechComponent>, warnings: &mut Vec<String>) {
    let checks: Vec<(PathBuf, &str)> = vec![
        (root.join("requirements.txt"), "python_requirements"),
        (root.join("pyproject.toml"), "python_pyproject"),
        (root.join("Pipfile"), "python_pipfile"),
        (root.join("pom.xml"), "java_maven"),
        (root.join("build.gradle"), "java_gradle"),
        (root.join("build.gradle.kts"), "java_gradle"),
        (root.join("go.mod"), "go_mod"),
        (root.join("Gemfile"), "ruby_gemfile"),
        (root.join("Dockerfile"), "dockerfile"),
        (root.join("docker-compose.yml"), "docker_compose"),
        (root.join("docker-compose.yaml"), "docker_compose"),
    ];

    for (path, kind) in checks {
        if !path.exists() {
            continue;
        }
        let raw = match fs::read_to_string(&path) {
            Ok(v) => v,
            Err(_) => {
                warnings.push(format!("读取失败: {}", path.to_string_lossy()));
                continue;
            }
        };
        match kind {
            "python_requirements" | "python_pyproject" | "python_pipfile" => {
                detect_python_from_text(&raw, detected, &path);
            }
            "java_maven" | "java_gradle" => {
                detect_java_from_text(&raw, detected, &path);
            }
            "go_mod" => {
                detect_go_from_text(&raw, detected, &path);
            }
            "ruby_gemfile" => {
                detect_ruby_from_text(&raw, detected, &path);
            }
            "dockerfile" | "docker_compose" => {
                detect_docker_from_text(&raw, detected, &path);
            }
            _ => {}
        }
    }

    let prisma = root.join("prisma").join("schema.prisma");
    if prisma.exists() {
        if let Ok(raw) = fs::read_to_string(&prisma) {
            detect_prisma_from_text(&raw, detected, &prisma);
        }
    }
}

fn detect_python_from_text(raw: &str, detected: &mut Vec<TechComponent>, path: &Path) {
    detected.push(TechComponent {
        id: "python".to_string(),
        name: "Python".to_string(),
        category: "backend".to_string(),
        version: None,
        confidence: 0.7,
        evidence: vec![format!("检测到 Python 配置: {}", path.file_name().unwrap().to_string_lossy())],
    });
    let text = raw.to_lowercase();
    if text.contains("django") {
        detected.push(TechComponent {
            id: "django".to_string(),
            name: "Django".to_string(),
            category: "backend".to_string(),
            version: None,
            confidence: 0.88,
            evidence: vec![format!("{}: django", path.file_name().unwrap().to_string_lossy())],
        });
    }
    if text.contains("fastapi") {
        detected.push(TechComponent {
            id: "fastapi".to_string(),
            name: "FastAPI".to_string(),
            category: "backend".to_string(),
            version: None,
            confidence: 0.88,
            evidence: vec![format!("{}: fastapi", path.file_name().unwrap().to_string_lossy())],
        });
    }
    if text.contains("flask") {
        detected.push(TechComponent {
            id: "flask".to_string(),
            name: "Flask".to_string(),
            category: "backend".to_string(),
            version: None,
            confidence: 0.85,
            evidence: vec![format!("{}: flask", path.file_name().unwrap().to_string_lossy())],
        });
    }
    if text.contains("pytest") {
        detected.push(TechComponent {
            id: "pytest".to_string(),
            name: "pytest".to_string(),
            category: "test".to_string(),
            version: None,
            confidence: 0.85,
            evidence: vec![format!("{}: pytest", path.file_name().unwrap().to_string_lossy())],
        });
    }
    if text.contains("sqlalchemy") {
        detected.push(TechComponent {
            id: "sqlalchemy".to_string(),
            name: "SQLAlchemy".to_string(),
            category: "database".to_string(),
            version: None,
            confidence: 0.8,
            evidence: vec![format!("{}: sqlalchemy", path.file_name().unwrap().to_string_lossy())],
        });
    }
    if text.contains("psycopg2") || text.contains("asyncpg") {
        detected.push(TechComponent {
            id: "postgres".to_string(),
            name: "PostgreSQL".to_string(),
            category: "database".to_string(),
            version: None,
            confidence: 0.75,
            evidence: vec![format!("{}: psycopg2/asyncpg", path.file_name().unwrap().to_string_lossy())],
        });
    }
    if text.contains("pymongo") {
        detected.push(TechComponent {
            id: "mongodb".to_string(),
            name: "MongoDB".to_string(),
            category: "database".to_string(),
            version: None,
            confidence: 0.75,
            evidence: vec![format!("{}: pymongo", path.file_name().unwrap().to_string_lossy())],
        });
    }
}

fn detect_java_from_text(raw: &str, detected: &mut Vec<TechComponent>, path: &Path) {
    detected.push(TechComponent {
        id: "java".to_string(),
        name: "Java".to_string(),
        category: "backend".to_string(),
        version: None,
        confidence: 0.7,
        evidence: vec![format!("检测到 Java 构建文件: {}", path.file_name().unwrap().to_string_lossy())],
    });
    let text = raw.to_lowercase();
    if text.contains("spring-boot") || text.contains("org.springframework.boot") {
        detected.push(TechComponent {
            id: "spring-boot".to_string(),
            name: "Spring Boot".to_string(),
            category: "backend".to_string(),
            version: None,
            confidence: 0.9,
            evidence: vec![format!("{}: spring boot", path.file_name().unwrap().to_string_lossy())],
        });
    }
    if text.contains("junit") {
        detected.push(TechComponent {
            id: "junit".to_string(),
            name: "JUnit".to_string(),
            category: "test".to_string(),
            version: None,
            confidence: 0.8,
            evidence: vec![format!("{}: junit", path.file_name().unwrap().to_string_lossy())],
        });
    }
    if text.contains("hibernate") {
        detected.push(TechComponent {
            id: "hibernate".to_string(),
            name: "Hibernate".to_string(),
            category: "database".to_string(),
            version: None,
            confidence: 0.78,
            evidence: vec![format!("{}: hibernate", path.file_name().unwrap().to_string_lossy())],
        });
    }
}

fn detect_go_from_text(raw: &str, detected: &mut Vec<TechComponent>, path: &Path) {
    detected.push(TechComponent {
        id: "go".to_string(),
        name: "Go".to_string(),
        category: "backend".to_string(),
        version: None,
        confidence: 0.75,
        evidence: vec![format!("检测到 go.mod: {}", path.file_name().unwrap().to_string_lossy())],
    });
    let text = raw.to_lowercase();
    if text.contains("github.com/gin-gonic/gin") {
        detected.push(TechComponent {
            id: "gin".to_string(),
            name: "Gin".to_string(),
            category: "backend".to_string(),
            version: None,
            confidence: 0.85,
            evidence: vec!["go.mod: gin-gonic/gin".to_string()],
        });
    }
    if text.contains("gorm.io/gorm") {
        detected.push(TechComponent {
            id: "gorm".to_string(),
            name: "GORM".to_string(),
            category: "database".to_string(),
            version: None,
            confidence: 0.8,
            evidence: vec!["go.mod: gorm.io/gorm".to_string()],
        });
    }
}

fn detect_ruby_from_text(raw: &str, detected: &mut Vec<TechComponent>, path: &Path) {
    detected.push(TechComponent {
        id: "ruby".to_string(),
        name: "Ruby".to_string(),
        category: "backend".to_string(),
        version: None,
        confidence: 0.65,
        evidence: vec![format!("检测到 Gemfile: {}", path.file_name().unwrap().to_string_lossy())],
    });
    let text = raw.to_lowercase();
    if text.contains("rails") {
        detected.push(TechComponent {
            id: "rails".to_string(),
            name: "Ruby on Rails".to_string(),
            category: "backend".to_string(),
            version: None,
            confidence: 0.85,
            evidence: vec!["Gemfile: rails".to_string()],
        });
    }
}

fn detect_docker_from_text(raw: &str, detected: &mut Vec<TechComponent>, path: &Path) {
    detected.push(TechComponent {
        id: "docker".to_string(),
        name: "Docker".to_string(),
        category: "infra".to_string(),
        version: None,
        confidence: 0.8,
        evidence: vec![format!("检测到容器配置: {}", path.file_name().unwrap().to_string_lossy())],
    });
    let text = raw.to_lowercase();
    for (id, name, keyword, confidence) in [
        ("postgres", "PostgreSQL", "postgres", 0.7),
        ("mysql", "MySQL", "mysql", 0.7),
        ("mariadb", "MariaDB", "mariadb", 0.7),
        ("mongodb", "MongoDB", "mongo", 0.7),
        ("redis", "Redis", "redis", 0.7),
    ] {
        if text.contains(keyword) {
            detected.push(TechComponent {
                id: id.to_string(),
                name: name.to_string(),
                category: "database".to_string(),
                version: None,
                confidence,
                evidence: vec![format!(
                    "{}: {}",
                    path.file_name().unwrap().to_string_lossy(),
                    keyword
                )],
            });
        }
    }
}

fn detect_prisma_from_text(raw: &str, detected: &mut Vec<TechComponent>, path: &Path) {
    let text = raw.to_lowercase();
    detected.push(TechComponent {
        id: "prisma".to_string(),
        name: "Prisma".to_string(),
        category: "database".to_string(),
        version: None,
        confidence: 0.9,
        evidence: vec![format!("检测到 Prisma schema: {}", path.to_string_lossy())],
    });
    if text.contains("provider = \"postgresql\"") {
        detected.push(TechComponent {
            id: "postgres".to_string(),
            name: "PostgreSQL".to_string(),
            category: "database".to_string(),
            version: None,
            confidence: 0.85,
            evidence: vec!["schema.prisma: provider=postgresql".to_string()],
        });
    }
    if text.contains("provider = \"mysql\"") {
        detected.push(TechComponent {
            id: "mysql".to_string(),
            name: "MySQL".to_string(),
            category: "database".to_string(),
            version: None,
            confidence: 0.85,
            evidence: vec!["schema.prisma: provider=mysql".to_string()],
        });
    }
    if text.contains("provider = \"mongodb\"") {
        detected.push(TechComponent {
            id: "mongodb".to_string(),
            name: "MongoDB".to_string(),
            category: "database".to_string(),
            version: None,
            confidence: 0.85,
            evidence: vec!["schema.prisma: provider=mongodb".to_string()],
        });
    }
}

fn collect_language_stats(
    root: &Path,
    options: &TechStackScanOptions,
    window: Option<&tauri::Window>,
    excluded_dirs: &[PathBuf],
) -> (HashMap<String, u64>, HashMap<String, usize>, usize, bool) {
    let mut bytes_by_lang: HashMap<String, u64> = HashMap::new();
    let mut files_by_lang: HashMap<String, usize> = HashMap::new();
    let mut processed_files = 0usize;
    let mut total_bytes = 0u64;
    let mut stopped_early = false;

    let excluded: Vec<PathBuf> = excluded_dirs
        .iter()
        .filter_map(|p| fs::canonicalize(p).ok())
        .collect();

    let mut builder = WalkBuilder::new(root);
    builder.hidden(false).parents(true);
    let walker = builder.filter_entry(move |entry| {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name == ".git" {
                return false;
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) && should_skip_dir(name) {
                return false;
            }
        }
        for ex in &excluded {
            if path.starts_with(ex) {
                return false;
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

        processed_files += 1;
        if processed_files % 400 == 0 {
            emit_progress(window, "统计语言", "采样统计中…", processed_files, None);
        }
        if processed_files > options.max_files {
            stopped_early = true;
            break;
        }

        let meta = match entry.metadata() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let size = meta.len();
        if size > 5 * 1024 * 1024 {
            continue;
        }
        total_bytes = total_bytes.saturating_add(size);
        if total_bytes > options.max_total_bytes {
            stopped_early = true;
            break;
        }

        let lang = language_from_extension(entry.path());
        *bytes_by_lang.entry(lang.clone()).or_insert(0) += size;
        *files_by_lang.entry(lang).or_insert(0) += 1;
    }

    (bytes_by_lang, files_by_lang, processed_files, stopped_early)
}

fn parse_git_submodules(root: &Path) -> Vec<GitSubmodule> {
    let path = root.join(".gitmodules");
    if !path.exists() {
        return Vec::new();
    }
    let raw = match fs::read_to_string(&path) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<GitSubmodule> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_path: Option<String> = None;
    let mut current_url: Option<String> = None;

    let flush = |out: &mut Vec<GitSubmodule>,
                 name: &mut Option<String>,
                 p: &mut Option<String>,
                 url: &mut Option<String>| {
        if let (Some(name), Some(path)) = (name.take(), p.take()) {
            out.push(GitSubmodule {
                name,
                path,
                url: url.take(),
                scanned: false,
                scan_warning: None,
            });
        } else {
            *name = None;
            *p = None;
            *url = None;
        }
    };

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            flush(&mut out, &mut current_name, &mut current_path, &mut current_url);
            let inner = &line[1..line.len() - 1];
            if let Some(rest) = inner.strip_prefix("submodule ") {
                let name = rest.trim().trim_matches('"').to_string();
                current_name = Some(name);
            }
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let value = v.trim().trim_matches('"').to_string();
            if key == "path" {
                current_path = Some(value);
            } else if key == "url" {
                current_url = Some(value);
            }
        }
    }
    flush(&mut out, &mut current_name, &mut current_path, &mut current_url);

    out.retain(|m| !m.path.trim().is_empty());
    out
}

fn open_cache_db() -> Option<Database> {
    let path = resolve_db_path().ok()?;
    Database::new(&path).ok()
}

fn build_scan_fingerprint(input_kind: &str, input_value: &str, root: &Path) -> String {
    let mut files: BTreeMap<String, Value> = BTreeMap::new();
    for rel in [
        "package.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "package-lock.json",
        "bun.lockb",
        "bun.lock",
        "requirements.txt",
        "pyproject.toml",
        "Pipfile",
        "Pipfile.lock",
        "poetry.lock",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "go.mod",
        "Gemfile",
        "Gemfile.lock",
        "Dockerfile",
        "docker-compose.yml",
        "docker-compose.yaml",
        ".gitmodules",
        "prisma/schema.prisma",
    ] {
        let p = root.join(rel);
        if !p.exists() {
            continue;
        }
        if let Ok(meta) = fs::metadata(&p) {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let size = meta.len();
            files.insert(
                rel.to_string(),
                serde_json::json!({ "mtime": mtime, "size": size }),
            );
        }
    }

    let git_head = read_git_head(root);
    let payload = serde_json::json!({
        "kind": input_kind,
        "value": input_value,
        "git": git_head,
        "files": files
    });
    serde_json::to_string(&payload).unwrap_or_else(|_| format!("{}|{}", input_kind, input_value))
}

fn read_git_head(root: &Path) -> Option<String> {
    let git_dir = root.join(".git");
    if !git_dir.exists() {
        return None;
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if head.is_empty() {
        None
    } else {
        Some(head)
    }
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "dist"
            | "build"
            | "out"
            | "target"
            | "vendor"
            | ".idea"
            | ".vscode"
            | ".next"
            | ".nuxt"
            | ".cache"
            | ".turbo"
            | ".pytest_cache"
            | "__pycache__"
    )
}

fn language_from_extension(path: &Path) -> String {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return "Other".to_string();
    };
    match ext.to_ascii_lowercase().as_str() {
        "ts" | "tsx" => "TypeScript".to_string(),
        "js" | "jsx" | "cjs" | "mjs" => "JavaScript".to_string(),
        "py" => "Python".to_string(),
        "java" => "Java".to_string(),
        "kt" | "kts" => "Kotlin".to_string(),
        "go" => "Go".to_string(),
        "rs" => "Rust".to_string(),
        "rb" => "Ruby".to_string(),
        "php" => "PHP".to_string(),
        "cs" => "C#".to_string(),
        "cpp" | "cc" | "cxx" | "c" | "h" | "hpp" => "C/C++".to_string(),
        "swift" => "Swift".to_string(),
        "html" | "htm" => "HTML".to_string(),
        "css" | "scss" | "sass" | "less" => "CSS".to_string(),
        "md" => "Markdown".to_string(),
        "json" | "yaml" | "yml" | "toml" | "xml" => "Config".to_string(),
        _ => "Other".to_string(),
    }
}

fn compute_language_percentages(
    bytes_by_lang: HashMap<String, u64>,
    files_by_lang: HashMap<String, usize>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parse_github_owner_repo() {
        let parsed = parse_github_input("https://github.com/owner/repo").unwrap();
        assert_eq!(parsed.owner, "owner");
        assert_eq!(parsed.repo, "repo");
        assert_eq!(parsed.reference, "HEAD");
        assert!(parsed.subdir.is_none());
    }

    #[test]
    fn parse_github_tree_ref_subdir() {
        let parsed = parse_github_input("owner/repo/tree/main/apps/web").unwrap();
        assert_eq!(parsed.owner, "owner");
        assert_eq!(parsed.repo, "repo");
        assert_eq!(parsed.reference, "main");
        assert_eq!(parsed.subdir.as_deref(), Some("apps/web"));
    }

    #[test]
    fn detect_react_vite_from_package_json() {
        let json = serde_json::json!({
            "dependencies": { "react": "^19.0.0" },
            "devDependencies": { "vite": "^7.0.0" },
            "scripts": { "dev": "vite" }
        });
        let mut detected = Vec::new();
        let mut graph = TechGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        analyze_package_json(&json, &mut detected, &mut graph);
        let has_react = detected.iter().any(|c| c.id == "react" && c.category == "frontend");
        let has_vite = detected.iter().any(|c| c.id == "vite" && c.category == "build");
        assert!(has_react);
        assert!(has_vite);
    }

    #[test]
    fn parse_gitmodules_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join(".gitmodules"),
            r#"
[submodule "a"]
  path = libs/a
  url = https://github.com/x/a.git
[submodule "b"]
  path = libs/b
  url = https://github.com/x/b.git
"#,
        )
        .unwrap();
        let mods = parse_git_submodules(root);
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].path, "libs/a");
        assert_eq!(mods[1].path, "libs/b");
    }

    #[test]
    fn perf_collect_language_stats_under_5s() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();
        fs::create_dir_all(root.join("node_modules")).unwrap();

        fs::write(
            root.join("package.json"),
            r#"{ "dependencies": { "react": "^19.0.0" }, "devDependencies": { "vite": "^7.0.0" } }"#,
        )
        .unwrap();

        for i in 0..2400 {
            let p = root.join("src").join(format!("file-{}.ts", i));
            fs::write(p, "export const x = 1;\n").unwrap();
        }

        let options = TechStackScanOptions::default();
        let start = Instant::now();
        let (_bytes, _files, processed, _stopped) =
            collect_language_stats(root, &options, None, &[]);
        assert!(processed > 0);
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
