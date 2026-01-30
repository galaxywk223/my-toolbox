import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Button } from "@/components/ui/button";
import { ToolLayout } from "@/components/ToolLayout";
import { Copy, FolderOpen, RefreshCw, Save } from "lucide-react";

type ProjectTreeScanResult = {
  tree: string;
  used_git: boolean;
  file_count: number;
  dir_count: number;
};

function normalizeRootPath(value: string) {
  return value.trim();
}

function buildSaveContent(tree: string, path: string) {
  const lower = path.toLowerCase();
  if (lower.endsWith(".md")) {
    return ["```text", tree, "```", ""].join("\n");
  }
  return tree;
}

export default function ProjectTreeScannerTool() {
  const [rootPath, setRootPath] = useState("");
  const [isScanning, setIsScanning] = useState(false);
  const [tree, setTree] = useState("");
  const [usedGit, setUsedGit] = useState<boolean | null>(null);
  const [counts, setCounts] = useState<{ files: number; dirs: number } | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const hasTree = tree.trim().length > 0;

  const summaryText = useMemo(() => {
    if (!counts) return "";
    const gitPart = usedGit === null ? "" : usedGit ? "（git 跟踪模式）" : "（全量扫描模式）";
    return `目录 ${counts.dirs} 个，文件 ${counts.files} 个 ${gitPart}`;
  }, [counts, usedGit]);

  const handlePickRoot = async () => {
    setStatus(null);
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择项目根目录",
    });
    if (!selected) return;
    if (Array.isArray(selected)) return;
    setRootPath(selected);
  };

  const handleScan = async () => {
    const normalized = normalizeRootPath(rootPath);
    if (!normalized) {
      setStatus("请输入或选择项目根目录");
      return;
    }

    setIsScanning(true);
    setStatus(null);
    try {
      const result = await invoke<ProjectTreeScanResult>("scan_project_tree", {
        root_path: normalized,
      });
      setTree(result.tree);
      setUsedGit(result.used_git);
      setCounts({ files: result.file_count, dirs: result.dir_count });
      setStatus("扫描完成");
    } catch (error) {
      setStatus(`扫描失败: ${String(error)}`);
    } finally {
      setIsScanning(false);
    }
  };

  const handleCopy = async () => {
    if (!hasTree) {
      setStatus("暂无可复制的目录树");
      return;
    }
    setStatus(null);
    try {
      await writeText(tree);
      setStatus("已复制到剪贴板");
    } catch (error) {
      setStatus(`复制失败: ${String(error)}`);
    }
  };

  const handleSave = async () => {
    if (!hasTree) {
      setStatus("暂无可保存的目录树");
      return;
    }
    setStatus(null);
    const target = await save({
      title: "保存目录树",
      defaultPath: "project-tree.txt",
      filters: [
        { name: "Text", extensions: ["txt"] },
        { name: "Markdown", extensions: ["md"] },
      ],
    });
    if (!target) return;

    try {
      const content = buildSaveContent(tree, target);
      await invoke("save_tree_to_file", { path: target, content });
      setStatus("已保存");
    } catch (error) {
      setStatus(`保存失败: ${String(error)}`);
    }
  };

  return (
    <ToolLayout
      title="项目目录结构扫描"
      description="扫描项目目录并生成标准树形结构；检测到 git 仓库时仅展示已跟踪内容。"
    >
      <div className="space-y-6">
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-4 items-end">
          <div className="lg:col-span-9 space-y-2">
            <div className="text-sm font-medium text-muted-foreground">项目根目录</div>
            <input
              value={rootPath}
              onChange={(e) => setRootPath(e.target.value)}
              placeholder="例如：D:\\Work\\my-project"
              className="w-full rounded-2xl border border-border/50 bg-background/40 px-4 py-3 text-sm outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary/40"
            />
          </div>
          <div className="lg:col-span-3 flex gap-2">
            <Button
              type="button"
              variant="outline"
              className="rounded-2xl flex-1"
              onClick={handlePickRoot}
              disabled={isScanning}
            >
              <FolderOpen className="w-4 h-4 mr-2" />
              选择目录
            </Button>
            <Button
              type="button"
              className="rounded-2xl flex-1"
              onClick={handleScan}
              disabled={isScanning}
            >
              <RefreshCw className={`w-4 h-4 mr-2 ${isScanning ? "animate-spin" : ""}`} />
              扫描
            </Button>
          </div>
        </div>

        <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-3">
          <div className="space-y-1">
            <div className="text-sm text-muted-foreground">
              {summaryText || "生成的目录树会显示在下方，可复制或保存。"}
            </div>
            {status && <div className="text-sm font-medium">{status}</div>}
          </div>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              className="rounded-2xl"
              onClick={handleCopy}
              disabled={!hasTree}
            >
              <Copy className="w-4 h-4 mr-2" />
              复制到剪贴板
            </Button>
            <Button
              type="button"
              variant="outline"
              className="rounded-2xl"
              onClick={handleSave}
              disabled={!hasTree}
            >
              <Save className="w-4 h-4 mr-2" />
              保存为文件
            </Button>
          </div>
        </div>

        <div className="rounded-2xl border border-border/50 bg-background/30 overflow-hidden">
          <div className="px-4 py-3 border-b border-border/40 text-sm font-medium">
            目录树预览
          </div>
          <pre className="p-4 text-xs leading-relaxed font-mono overflow-auto max-h-[520px]">
            {hasTree ? tree : "暂无结果。请先选择目录并点击“扫描”。"}
          </pre>
        </div>
      </div>
    </ToolLayout>
  );
}

