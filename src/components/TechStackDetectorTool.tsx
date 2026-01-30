import { useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { ToolLayout } from "@/components/ToolLayout";
import {
  Download,
  FileJson,
  FolderOpen,
  Link2,
  RefreshCw,
  StopCircle,
} from "lucide-react";
import ReactECharts from "echarts-for-react";
import sampleReport from "../../sample_report.json";

type LanguageStat = {
  language: string;
  bytes: number;
  files: number;
  percent: number;
};

type BackendStack = {
  framework: string;
  rest: boolean;
  db: string;
  queue: string;
  orm?: string | null;
  migrations?: string | null;
  aiFeatures?: string[];
};

type FrontendStack = {
  builder: string;
  vue: number | null;
  store: string;
  ui: string;
  visualization?: string[];
};

type GeneratedSummary = {
  files: number;
  klocIgnored: number;
};

type AssetSummary = {
  files: number;
  bytes: number;
};

type ModuleReport = {
  name: string;
  path: string;
  languages: LanguageStat[];
  frameworks: string[];
  deps: string[];
  backend: BackendStack | null;
  frontend: FrontendStack | null;
  generated: GeneratedSummary | null;
  assets: AssetSummary | null;
  warnings: string[];
};

type SemanticSummary = {
  totalSize: number;
  ignoredSize: number;
  ignoredRatio: number;
  assetsSize: number;
  generatedFiles: number;
  generatedKlocIgnored: number;
  effectiveFiles: number;
  effectiveKloc: number;
  warnings: string[];
};

type IgnoredFile = {
  path: string;
  size: number;
  reason: string;
  category: string;
};

type SemanticReport = {
  scanTimestamp: string;
  repoRoot: string;
  modules: ModuleReport[];
  summary: SemanticSummary;
  ignoredFiles: IgnoredFile[];
};

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let idx = 0;
  let n = bytes;
  while (n >= 1024 && idx < units.length - 1) {
    n /= 1024;
    idx += 1;
  }
  return `${n.toFixed(idx === 0 ? 0 : 1)} ${units[idx]}`;
}

function normalizeUrl(value: string) {
  return value.trim();
}

function normalizePath(value: string) {
  return value.trim();
}

function ratioText(ratio: number) {
  if (!Number.isFinite(ratio)) return "0.00%";
  return `${(ratio * 100).toFixed(2)}%`;
}

function chip(text: string) {
  return (
    <span className="px-2.5 py-1 rounded-full bg-background/40 border border-border/50 text-xs">
      {text}
    </span>
  );
}

export default function TechStackDetectorTool() {
  const [mode, setMode] = useState<"local" | "github">("local");
  const [localPath, setLocalPath] = useState("");
  const [githubUrl, setGithubUrl] = useState("");
  const [isScanning, setIsScanning] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [report, setReport] = useState<SemanticReport | null>(null);
  const [activeModule, setActiveModule] = useState<string | null>(null);

  const cancelTokenRef = useRef(0);

  const selectedModule = useMemo(() => {
    if (!report) return null;
    const key = activeModule ?? report.modules[0]?.path ?? null;
    if (!key) return null;
    return report.modules.find((m) => m.path === key) ?? report.modules[0] ?? null;
  }, [activeModule, report]);

  const languageChartOption = useMemo(() => {
    if (!selectedModule) return null;
    const sorted = [...selectedModule.languages].sort((a, b) => b.bytes - a.bytes);
    const top = sorted.slice(0, 8);
    const restBytes = sorted.slice(8).reduce((sum, s) => sum + s.bytes, 0);
    const data = top.map((s) => ({ name: s.language, value: s.bytes }));
    if (restBytes > 0) data.push({ name: "Other", value: restBytes });
    return {
      tooltip: { trigger: "item" },
      series: [
        {
          type: "pie",
          radius: ["35%", "70%"],
          avoidLabelOverlap: true,
          itemStyle: {
            borderRadius: 8,
            borderWidth: 2,
            borderColor: "rgba(0,0,0,0)",
          },
          label: { show: true, formatter: "{b}\n{d}%" },
          data,
        },
      ],
    };
  }, [selectedModule]);

  const noiseBadge = useMemo(() => {
    if (!report) return null;
    const ok = report.summary.ignoredRatio <= 0.1;
    const cls = ok
      ? "bg-emerald-500/10 border-emerald-500/20 text-emerald-500"
      : "bg-red-500/10 border-red-500/20 text-red-500";
    return (
      <div className={`inline-flex items-center gap-2 px-3 py-2 rounded-2xl border ${cls}`}>
        <span className="text-xs font-medium">噪音占比</span>
        <span className="text-sm font-mono font-bold">{ratioText(report.summary.ignoredRatio)}</span>
      </div>
    );
  }, [report]);

  const handlePickRoot = async () => {
    setStatus(null);
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择项目根目录",
    });
    if (!selected || Array.isArray(selected)) return;
    setLocalPath(selected);
  };

  const loadSample = () => {
    const r = sampleReport as unknown as SemanticReport;
    setReport(r);
    setActiveModule(r.modules[0]?.path ?? null);
    setStatus("已加载样例数据");
  };

  const handleScan = async () => {
    setStatus(null);
    setReport(null);

    const token = Date.now();
    cancelTokenRef.current = token;

    setIsScanning(true);
    try {
      if (mode === "local") {
        const rootPath = normalizePath(localPath);
        if (!rootPath) {
          setStatus("请输入或选择本地项目路径");
          return;
        }
        const result = await invoke<SemanticReport>("scan_semantic_local", {
          rootPath,
        });
        if (cancelTokenRef.current !== token) return;
        setReport(result);
        setActiveModule(result.modules[0]?.path ?? null);
        setStatus("扫描完成");
        return;
      }

      const repoUrl = normalizeUrl(githubUrl);
      if (!repoUrl) {
        setStatus("请输入 GitHub 仓库链接");
        return;
      }
      const result = await invoke<SemanticReport>("scan_semantic_github", { repoUrl });
      if (cancelTokenRef.current !== token) return;
      setReport(result);
      setActiveModule(result.modules[0]?.path ?? null);
      setStatus("扫描完成");
    } catch (error) {
      if (cancelTokenRef.current !== token) return;
      setStatus(`扫描失败: ${String(error)}`);
    } finally {
      if (cancelTokenRef.current !== token) return;
      setIsScanning(false);
    }
  };

  const handleCancel = () => {
    cancelTokenRef.current = Date.now();
    setIsScanning(false);
    setStatus("已取消（后台任务可能仍在运行）");
  };

  const handleExportJson = async () => {
    if (!report) return;
    setStatus(null);
    const target = await save({
      title: "导出 JSON 报告",
      defaultPath: "semantic-report.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!target) return;
    try {
      await invoke("export_semantic_json", { path: target, report });
      setStatus("已导出 JSON");
    } catch (error) {
      setStatus(`导出失败: ${String(error)}`);
    }
  };

  const summaryText = useMemo(() => {
    if (!report) return null;
    const parts: string[] = [];
    parts.push(`Repo: ${report.repoRoot}`);
    parts.push(`Modules: ${report.modules.length}`);
    parts.push(`Ignored: ${formatBytes(report.summary.ignoredSize)}`);
    parts.push(`Total: ${formatBytes(report.summary.totalSize)}`);
    return parts.join(" · ");
  }, [report]);

  return (
    <ToolLayout
      title="技术栈探测（语义版）"
      description="解析生态配置并按模块输出技术栈；支持 Monorepo 模式、噪音过滤与生成代码降权。"
    >
      <div className="space-y-6">
        <div className="flex gap-2">
          <Button
            type="button"
            variant={mode === "local" ? "default" : "outline"}
            className="rounded-2xl"
            onClick={() => setMode("local")}
            disabled={isScanning}
          >
            <FolderOpen className="w-4 h-4 mr-2" />
            本地项目
          </Button>
          <Button
            type="button"
            variant={mode === "github" ? "default" : "outline"}
            className="rounded-2xl"
            onClick={() => setMode("github")}
            disabled={isScanning}
          >
            <Link2 className="w-4 h-4 mr-2" />
            GitHub 链接
          </Button>
          <Button
            type="button"
            variant="outline"
            className="rounded-2xl"
            onClick={loadSample}
            disabled={isScanning}
          >
            <FileJson className="w-4 h-4 mr-2" />
            加载样例
          </Button>
        </div>

        {mode === "local" ? (
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-4 items-end">
            <div className="lg:col-span-9 space-y-2">
              <div className="text-sm font-medium text-muted-foreground">项目根目录</div>
              <input
                value={localPath}
                onChange={(e) => setLocalPath(e.target.value)}
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
        ) : (
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-4 items-end">
            <div className="lg:col-span-9 space-y-2">
              <div className="text-sm font-medium text-muted-foreground">GitHub 仓库链接</div>
              <input
                value={githubUrl}
                onChange={(e) => setGithubUrl(e.target.value)}
                placeholder="例如：https://github.com/owner/repo 或 .../tree/main/subdir"
                className="w-full rounded-2xl border border-border/50 bg-background/40 px-4 py-3 text-sm outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary/40"
              />
            </div>
            <div className="lg:col-span-3 flex gap-2">
              <Button
                type="button"
                className="rounded-2xl flex-1"
                onClick={handleScan}
                disabled={isScanning}
              >
                <RefreshCw className={`w-4 h-4 mr-2 ${isScanning ? "animate-spin" : ""}`} />
                扫描
              </Button>
              <Button
                type="button"
                variant="outline"
                className="rounded-2xl"
                onClick={handleCancel}
                disabled={!isScanning}
              >
                <StopCircle className="w-4 h-4" />
              </Button>
            </div>
          </div>
        )}

        <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-3">
          <div className="space-y-1">
            <div className="text-sm text-muted-foreground">
              {summaryText || "按模块输出语言分布与技术栈摘要；默认过滤 lock/map/min.js 等噪音文件。"}
            </div>
            {status && <div className="text-sm font-medium">{status}</div>}
          </div>
          <div className="flex gap-2 items-center">
            {noiseBadge}
            <Button
              type="button"
              variant="outline"
              className="rounded-2xl"
              onClick={handleCancel}
              disabled={!isScanning}
            >
              <StopCircle className="w-4 h-4 mr-2" />
              取消
            </Button>
          </div>
        </div>

        <div className="rounded-2xl border border-border/50 bg-background/30 overflow-hidden">
          <div className="px-4 py-3 border-b border-border/40 flex items-center justify-between gap-3">
            <div className="text-sm font-medium">模块化视图</div>
            <div className="flex gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                className="rounded-2xl"
                onClick={handleExportJson}
                disabled={!report}
              >
                <Download className="w-4 h-4 mr-2" />
                JSON
              </Button>
            </div>
          </div>

          <div className="p-4">
            {!report || !selectedModule ? (
              <div className="text-sm text-muted-foreground">
                暂无结果。请扫描或加载样例数据。
              </div>
            ) : (
              <div className="grid grid-cols-1 lg:grid-cols-12 gap-4">
                <div className="lg:col-span-3 space-y-2">
                  <div className="text-sm font-medium">Modules</div>
                  <div className="space-y-2">
                    {report.modules.map((m) => (
                      <button
                        key={m.path}
                        type="button"
                        onClick={() => setActiveModule(m.path)}
                        className={`w-full text-left rounded-2xl border px-3 py-2 transition-colors ${
                          m.path === selectedModule.path
                            ? "border-primary/40 bg-primary/10"
                            : "border-border/50 bg-background/20 hover:bg-background/30"
                        }`}
                      >
                        <div className="text-sm font-medium">{m.name}</div>
                        <div className="text-xs text-muted-foreground">{m.path}</div>
                      </button>
                    ))}
                  </div>

                  <div className="rounded-2xl border border-border/50 bg-background/20 p-3 space-y-2">
                    <div className="text-sm font-medium">Summary</div>
                    <div className="text-xs text-muted-foreground">
                      Total: {formatBytes(report.summary.totalSize)}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      Ignored: {formatBytes(report.summary.ignoredSize)}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      Assets: {formatBytes(report.summary.assetsSize)}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      Generated: {report.summary.generatedFiles} files ·{" "}
                      {report.summary.generatedKlocIgnored.toFixed(1)} kLOC
                    </div>
                  </div>
                </div>

                <div className="lg:col-span-9 space-y-4">
                  <div className="rounded-2xl border border-border/50 bg-background/20 p-4 space-y-3">
                    <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-3">
                      <div className="space-y-1">
                        <div className="text-sm font-medium">
                          {selectedModule.name} ({selectedModule.path})
                        </div>
                        <div className="text-xs text-muted-foreground">
                          Scan: {report.scanTimestamp}
                        </div>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {selectedModule.frameworks.map((t) => (
                          <span key={`fw:${t}`}>{chip(t)}</span>
                        ))}
                        {selectedModule.deps.map((t) => (
                          <span key={`dep:${t}`}>{chip(t)}</span>
                        ))}
                      </div>
                    </div>

                    {selectedModule.backend ? (
                      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                        <div className="rounded-2xl border border-border/50 bg-background/30 p-3 space-y-1">
                          <div className="text-sm font-medium">Backend</div>
                          <div className="text-xs text-muted-foreground">
                            Framework: {selectedModule.backend.framework}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            REST: {selectedModule.backend.rest ? "Yes" : "No"}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            DB: {selectedModule.backend.db}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            Queue: {selectedModule.backend.queue}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            ORM: {selectedModule.backend.orm || "None"}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            Migrations: {selectedModule.backend.migrations || "None"}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            AI:{" "}
                            {selectedModule.backend.aiFeatures?.length
                              ? selectedModule.backend.aiFeatures.join(", ")
                              : "None"}
                          </div>
                        </div>
                      </div>
                    ) : null}

                    {selectedModule.frontend ? (
                      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                        <div className="rounded-2xl border border-border/50 bg-background/30 p-3 space-y-1">
                          <div className="text-sm font-medium">Frontend</div>
                          <div className="text-xs text-muted-foreground">
                            Builder: {selectedModule.frontend.builder}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            Vue:{" "}
                            {selectedModule.frontend.vue != null
                              ? `v${selectedModule.frontend.vue}`
                              : "None"}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            Store: {selectedModule.frontend.store}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            UI: {selectedModule.frontend.ui}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            Visualization:{" "}
                            {selectedModule.frontend.visualization?.length
                              ? selectedModule.frontend.visualization.join(", ")
                              : "None"}
                          </div>
                        </div>
                      </div>
                    ) : null}

                    <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                      <div className="rounded-2xl border border-border/50 bg-background/30 p-3 space-y-1">
                        <div className="text-sm font-medium">Generated</div>
                        <div className="text-xs text-muted-foreground">
                          {selectedModule.generated
                            ? `${selectedModule.generated.files} files · ${selectedModule.generated.klocIgnored.toFixed(1)} kLOC`
                            : "None"}
                        </div>
                      </div>
                      <div className="rounded-2xl border border-border/50 bg-background/30 p-3 space-y-1">
                        <div className="text-sm font-medium">Assets</div>
                        <div className="text-xs text-muted-foreground">
                          {selectedModule.assets
                            ? `${selectedModule.assets.files} files · ${formatBytes(selectedModule.assets.bytes)}`
                            : "None"}
                        </div>
                      </div>
                      <div className="rounded-2xl border border-border/50 bg-background/30 p-3 space-y-1">
                        <div className="text-sm font-medium">Noise</div>
                        <div className="text-xs text-muted-foreground">
                          {ratioText(report.summary.ignoredRatio)}
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="rounded-2xl border border-border/50 bg-background/20 p-4 space-y-3">
                    <div className="text-sm font-medium">语言占比（模块）</div>
                    {languageChartOption ? (
                      <ReactECharts
                        option={languageChartOption}
                        style={{ height: 260, width: "100%" }}
                        notMerge
                        lazyUpdate
                      />
                    ) : null}
                    <div className="space-y-2">
                      {selectedModule.languages.slice(0, 12).map((l) => (
                        <div key={l.language} className="flex items-center justify-between text-sm">
                          <div className="flex items-center gap-2">
                            <span className="font-medium">{l.language}</span>
                            <span className="text-muted-foreground">
                              {l.files} 文件 · {formatBytes(l.bytes)}
                            </span>
                          </div>
                          <div className="tabular-nums">{l.percent.toFixed(1)}%</div>
                        </div>
                      ))}
                    </div>
                  </div>

                  {selectedModule.warnings.length ? (
                    <div className="rounded-2xl border border-border/50 bg-background/20 p-4 space-y-2">
                      <div className="text-sm font-medium">Warnings</div>
                      <ul className="text-sm text-muted-foreground list-disc pl-5 space-y-1">
                        {selectedModule.warnings.map((w, idx) => (
                          <li key={idx}>{w}</li>
                        ))}
                      </ul>
                    </div>
                  ) : null}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </ToolLayout>
  );
}
