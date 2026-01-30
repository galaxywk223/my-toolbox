use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    Dir,
    File,
}

struct Node {
    kind: NodeKind,
    children: BTreeMap<String, Node>,
}

impl Node {
    fn dir() -> Self {
        Self {
            kind: NodeKind::Dir,
            children: BTreeMap::new(),
        }
    }

    fn file() -> Self {
        Self {
            kind: NodeKind::File,
            children: BTreeMap::new(),
        }
    }
}

#[derive(Serialize)]
pub struct ProjectTreeScanResult {
    pub tree: String,
    pub used_git: bool,
    pub file_count: usize,
    pub dir_count: usize,
}

#[tauri::command]
pub async fn scan_project_tree(root_path: String) -> Result<ProjectTreeScanResult, String> {
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

    tokio::task::spawn_blocking(move || scan_project_tree_blocking(&root))
        .await
        .map_err(|e| format!("扫描任务失败: {}", e))?
}

fn scan_project_tree_blocking(root: &Path) -> Result<ProjectTreeScanResult, String> {
    let mut tree = Node::dir();

    let (used_git, file_paths) = get_git_tracked_files(root).unwrap_or((false, Vec::new()));

    if used_git {
        for rel in file_paths {
            insert_posix_path(&mut tree, &rel, NodeKind::File);
        }
    } else {
        walk_filesystem(root, root, &mut tree)?;
    }

    let (dir_count, file_count) = count_nodes(&tree);
    let mut lines = Vec::new();
    lines.push("[D] .".to_string());
    render_children(&tree, "", &mut Vec::new(), &mut lines);

    Ok(ProjectTreeScanResult {
        tree: lines.join("\n"),
        used_git,
        file_count,
        dir_count,
    })
}

fn get_git_tracked_files(root: &Path) -> Option<(bool, Vec<String>)> {
    let git_marker = root.join(".git");
    if !git_marker.exists() {
        return None;
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("ls-files")
        .arg("-z")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = output.stdout;
    if stdout.is_empty() {
        return Some((true, Vec::new()));
    }

    let mut files = Vec::new();
    for part in stdout.split(|b| *b == 0u8) {
        if part.is_empty() {
            continue;
        }
        let raw = String::from_utf8_lossy(part).trim().to_string();
        if raw.is_empty() {
            continue;
        }
        files.push(raw);
    }

    Some((true, files))
}

fn walk_filesystem(root: &Path, current: &Path, tree: &mut Node) -> Result<(), String> {
    let entries = match fs::read_dir(current) {
        Ok(read_dir) => read_dir,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let rel = match path.strip_prefix(root) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if rel.as_os_str().is_empty() {
            continue;
        }

        if file_type.is_dir() {
            let rel_posix = to_posix_relative(rel);
            insert_posix_path(tree, &rel_posix, NodeKind::Dir);
            walk_filesystem(root, &path, tree)?;
        } else if file_type.is_file() {
            let rel_posix = to_posix_relative(rel);
            insert_posix_path(tree, &rel_posix, NodeKind::File);
        }
    }

    Ok(())
}

fn to_posix_relative(path: &Path) -> String {
    let mut out = String::new();
    for comp in path.components() {
        let part = comp.as_os_str().to_string_lossy();
        if part.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(&part);
    }
    out
}

fn insert_posix_path(tree: &mut Node, posix: &str, kind: NodeKind) {
    let mut current = tree;
    let parts: Vec<&str> = posix.split('/').filter(|p| !p.is_empty()).collect();
    for (idx, name) in parts.iter().enumerate() {
        let is_last = idx + 1 == parts.len();
        let next_kind = if is_last { kind } else { NodeKind::Dir };

        current = current
            .children
            .entry((*name).to_string())
            .and_modify(|node| {
                if node.kind == NodeKind::File && next_kind == NodeKind::Dir {
                    node.kind = NodeKind::Dir;
                }
            })
            .or_insert_with(|| match next_kind {
                NodeKind::Dir => Node::dir(),
                NodeKind::File => Node::file(),
            });
    }
}

fn count_nodes(node: &Node) -> (usize, usize) {
    let mut dir_count = 0usize;
    let mut file_count = 0usize;
    for child in node.children.values() {
        match child.kind {
            NodeKind::Dir => dir_count += 1,
            NodeKind::File => file_count += 1,
        }
        let (d, f) = count_nodes(child);
        dir_count += d;
        file_count += f;
    }
    (dir_count, file_count)
}

fn render_children(node: &Node, prefix: &str, path_stack: &mut Vec<String>, lines: &mut Vec<String>) {
    let mut children: Vec<(&String, &Node)> = node.children.iter().collect();
    children.sort_by(|(a_name, a_node), (b_name, b_node)| {
        match (a_node.kind, b_node.kind) {
            (NodeKind::Dir, NodeKind::File) => std::cmp::Ordering::Less,
            (NodeKind::File, NodeKind::Dir) => std::cmp::Ordering::Greater,
            _ => a_name.cmp(b_name),
        }
    });

    for (index, (name, child)) in children.iter().enumerate() {
        let is_last = index + 1 == children.len();
        let branch = if is_last { "└── " } else { "├── " };
        let next_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

        path_stack.push((**name).clone());
        let rel_path = path_stack.join("/");
        let line = match child.kind {
            NodeKind::Dir => format!("{}{}[D] {}/", prefix, branch, rel_path),
            NodeKind::File => format!("{}{}[F] {}", prefix, branch, rel_path),
        };
        lines.push(line);

        if child.kind == NodeKind::Dir {
            render_children(child, &next_prefix, path_stack, lines);
        }
        path_stack.pop();
    }
}

#[tauri::command]
pub async fn save_tree_to_file(path: String, content: String) -> Result<(), String> {
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err("请选择保存路径".to_string());
    }

    tokio::task::spawn_blocking(move || save_tree_to_file_blocking(&path, &content))
        .await
        .map_err(|e| format!("保存任务失败: {}", e))?
}

fn save_tree_to_file_blocking(path: &str, content: &str) -> Result<(), String> {
    let target = PathBuf::from(path);
    let ext = target
        .extension()
        .and_then(|v| v.to_str())
        .map(|s| s.to_ascii_lowercase())
        .ok_or_else(|| "仅支持保存为 .txt 或 .md 文件".to_string())?;

    if ext != "txt" && ext != "md" {
        return Err("仅支持保存为 .txt 或 .md 文件".to_string());
    }

    let parent = target
        .parent()
        .ok_or_else(|| "保存路径无效".to_string())?;
    if !parent.exists() {
        return Err("保存目录不存在".to_string());
    }

    let file_name = target
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| "保存路径无效".to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "无法生成临时文件名".to_string())?
        .as_nanos();
    let tmp_name = format!(".{}.{}.tmp", file_name, nonce);
    let tmp_path = parent.join(tmp_name);

    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&tmp_path)
        .map_err(|e| format!("无法写入文件: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入失败: {}", e))?;
    file.sync_all().map_err(|e| format!("写入失败: {}", e))?;
    drop(file);

    if target.exists() {
        let _ = fs::remove_file(&target);
    }

    fs::rename(&tmp_path, &target).map_err(|e| format!("保存失败: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_tree_with_full_relative_paths() {
        let mut root = Node::dir();
        insert_posix_path(&mut root, "src/main.tsx", NodeKind::File);
        insert_posix_path(&mut root, "src/components/ToolLayout.tsx", NodeKind::File);
        insert_posix_path(&mut root, "README.md", NodeKind::File);
        insert_posix_path(&mut root, "empty-dir", NodeKind::Dir);

        let mut lines = Vec::new();
        lines.push("[D] .".to_string());
        render_children(&root, "", &mut Vec::new(), &mut lines);
        let rendered = lines.join("\n");

        let expected = [
            "[D] .",
            "├── [D] empty-dir/",
            "├── [D] src/",
            "│   ├── [D] src/components/",
            "│   │   └── [F] src/components/ToolLayout.tsx",
            "│   └── [F] src/main.tsx",
            "└── [F] README.md",
        ]
        .join("\n");

        assert_eq!(rendered, expected);
    }

    #[test]
    fn detects_git_and_lists_tracked_files() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();

        let ok = Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            return;
        }

        fs::write(root.join("a.txt"), "hello").expect("write a.txt");
        let _ = Command::new("git")
            .arg("add")
            .arg("a.txt")
            .current_dir(root)
            .output();

        let (used_git, files) = get_git_tracked_files(root).unwrap_or((false, Vec::new()));
        assert!(used_git);
        assert!(files.iter().any(|f| f == "a.txt"));
    }
}
