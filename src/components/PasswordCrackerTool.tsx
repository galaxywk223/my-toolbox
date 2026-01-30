import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Play,
  Terminal,
  Activity,
  Cpu,
  Calendar,
  User,
  FileUp,
  CheckCircle2,
  Edit2,
  Hash,
  StopCircle,
  Trash2,
  History,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ToolLayout } from "@/components/ToolLayout";

// --- Types ---
interface CrackProgress {
  current_password: string;
  total_attempted: number;
  total_passwords: number;
  found: boolean;
  result?: string;
  elapsed_seconds: number;
}

interface PasswordResult {
  id: number;
  username: string;
  name?: string | null;
  class_name?: string | null;
  password_date?: string | null;
  created_at: string;
}

// --- Components ---

const StatusBadge = ({
  label,
  value,
  active = false,
}: {
  label: string;
  value: string | number;
  active?: boolean;
}) => (
  <div
    className={`flex flex-col p-3 rounded-lg border transition-all ${active ? "bg-primary/10 border-primary/30" : "bg-muted/20 border-border/50"}`}
  >
    <span className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
      {label}
    </span>
    <span
      className={`text-lg font-mono font-bold ${active ? "text-primary" : "text-foreground"}`}
    >
      {value}
    </span>
  </div>
);

const TerminalLog = ({ logs }: { logs: string[] }) => {
  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  return (
    <div className="h-64 bg-black/90 rounded-xl border border-white/10 p-4 font-mono text-xs overflow-y-auto shadow-inner relative">
      <div className="absolute top-2 right-2 flex gap-1.5">
        <div className="w-2.5 h-2.5 rounded-full bg-red-500/50" />
        <div className="w-2.5 h-2.5 rounded-full bg-amber-500/50" />
        <div className="w-2.5 h-2.5 rounded-full bg-green-500/50" />
      </div>
      <div className="text-emerald-500/50 mb-2 select-none">
        $ 日志监控中
      </div>
      {logs.length === 0 && (
        <span className="text-muted-foreground/50 italic">
          等待任务开始...
        </span>
      )}
      {logs.map((log, i) => (
        <div
          key={i}
          className="text-emerald-400 break-all leading-relaxed border-l-2 border-transparent hover:border-emerald-500/30 hover:bg-white/5 pl-2 transition-colors"
        >
          <span className="text-emerald-700 mr-2">
            [{new Date().toLocaleTimeString()}]
          </span>
          {log}
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );
};

// --- Main Component ---

export default function PasswordCrackerTool() {
  // State
  const [username, setUsername] = useState("239074295");
  const [year, setYear] = useState(2005);
  const [concurrency, setConcurrency] = useState(10);
  const [isRunning, setIsRunning] = useState(false);
  const [progress, setProgress] = useState<CrackProgress | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [history, setHistory] = useState<PasswordResult[]>([]);

  useEffect(() => {
    loadHistory();
    const unlisten = listen<CrackProgress>("crack_progress", (event) => {
      setProgress(event.payload);
      if (event.payload.found) {
        addLog(`成功：已找到密码，结果：${event.payload.result}`);
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const addLog = (msg: string) => {
    setLogs((prev) => [...prev.slice(-100), msg]); // Keep last 100 logs
  };

  const loadHistory = async () => {
    try {
      const result = await invoke<PasswordResult[]>("get_crack_history");
      setHistory(result);
    } catch (error) {
      console.error("Failed to load history:", error);
    }
  };

  const handleCrack = async () => {
    if (!username.trim()) return addLog("错误：请输入学号。");

    setIsRunning(true);
    setLogs([
      "正在初始化任务...",
      `目标：${username}`,
      `并发数：${concurrency}`,
    ]);
    setProgress(null);

    try {
      const result = await invoke<string>("crack_password", {
        request: {
          username: username.trim(),
          name: null,
          year,
          concurrency,
        },
      });
      addLog(`完成：${result}`);
      loadHistory();
    } catch (error) {
      addLog(`失败：${error}`);
    } finally {
      setIsRunning(false);
    }
  };

  // Import Logic (Simplified for brevity)
  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    // ... reuse existing logic logic ...
    addLog(`已选择文件：${file.name}（导入逻辑暂未启用）`);
  };

  const handleEditRecord = async (record: PasswordResult) => {
    const nextName = window.prompt("姓名", record.name ?? "");
    if (nextName === null) return;
    const nextClass = window.prompt("班级", record.class_name ?? "");
    if (nextClass === null) return;
    const nextPassword = window.prompt("密码/日期", record.password_date ?? "");
    if (nextPassword === null) return;
    try {
      await invoke("update_password_result", {
        username: record.username,
        name: nextName,
        class_name: nextClass,
        password_date: nextPassword,
      });
      await loadHistory();
    } catch (error) {
      alert(`更新失败: ${error}`);
    }
  };

  const handleDeleteRecord = async (record: PasswordResult) => {
    if (!window.confirm(`确定删除 ${record.username} 吗？`)) return;
    try {
      await invoke("delete_password_result", { username: record.username });
      await loadHistory();
    } catch (error) {
      alert(`删除失败: ${error}`);
    }
  };

  const progressPercent = progress
    ? Math.round((progress.total_attempted / progress.total_passwords) * 100)
    : 0;

  return (
    <ToolLayout
      title="教务系统探针"
      description="高级教务系统日期节点验证与查询工具 (v2.0)"
      actions={
        <Button variant="outline" size="sm" onClick={loadHistory}>
          <History className="w-4 h-4 mr-2" />
          刷新记录
        </Button>
      }
    >
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 h-full">
        {/* Left Panel: Controls */}
        <div className="lg:col-span-4 space-y-6">
          <div className="bg-card border border-border/50 rounded-2xl p-6 shadow-sm space-y-6">
            <div className="flex items-center gap-2 pb-4 border-b border-border/50">
              <Activity className="w-5 h-5 text-primary" />
              <h3 className="font-semibold">控制台</h3>
            </div>

            <div className="space-y-4">
              <div className="space-y-2">
                <label className="text-xs font-medium text-muted-foreground uppercase">
                  学号
                </label>
                <div className="relative">
                  <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                  <input
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    className="w-full bg-muted/30 border border-border rounded-lg pl-9 pr-4 py-2.5 font-mono text-sm focus:ring-1 focus:ring-primary outline-none transition-all"
                    placeholder="ex: 20210001"
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <label className="text-xs font-medium text-muted-foreground uppercase">
                    年份
                  </label>
                  <div className="relative">
                    <Calendar className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                    <input
                      type="number"
                      value={year}
                      onChange={(e) => setYear(parseInt(e.target.value))}
                      className="w-full bg-muted/30 border border-border rounded-lg pl-9 pr-2 py-2.5 font-mono text-sm focus:ring-1 focus:ring-primary outline-none transition-all"
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium text-muted-foreground uppercase">
                    线程数
                  </label>
                  <div className="relative">
                    <Cpu className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                    <input
                      type="number"
                      value={concurrency}
                      onChange={(e) => setConcurrency(parseInt(e.target.value))}
                      className="w-full bg-muted/30 border border-border rounded-lg pl-9 pr-2 py-2.5 font-mono text-sm focus:ring-1 focus:ring-primary outline-none transition-all"
                    />
                  </div>
                </div>
              </div>
            </div>

            <Button
              className={`w-full h-12 text-lg font-bold tracking-wide transition-all ${isRunning ? "bg-destructive/10 text-destructive hover:bg-destructive/20 border border-destructive/20" : "bg-primary text-primary-foreground shadow-lg shadow-primary/20 hover:scale-[1.02]"}`}
              onClick={handleCrack}
              disabled={isRunning}
            >
              {isRunning ? (
                <>
                  <StopCircle className="w-5 h-5 mr-2 animate-pulse" />
                  停止
                </>
              ) : (
                <>
                  <Play className="w-5 h-5 mr-2" />
                  开始
                </>
              )}
            </Button>
          </div>

          {/* Quick Import Card */}
          <div className="bg-card/50 border border-border/50 rounded-2xl p-6 space-y-4">
            <h4 className="text-sm font-semibold flex items-center gap-2">
              <FileUp className="w-4 h-4" /> 批量导入
            </h4>
            <div className="grid grid-cols-2 gap-3">
              <label className="cursor-pointer border border-dashed border-border hover:border-primary/50 hover:bg-primary/5 rounded-xl p-4 flex flex-col items-center justify-center gap-2 transition-all">
                <input
                  type="file"
                  className="hidden"
                  accept=".xlsx"
                  onChange={handleImportFile}
                />
                <User className="w-5 h-5 text-muted-foreground" />
                <span className="text-xs text-muted-foreground">
                  学生信息.xlsx
                </span>
              </label>
              <label className="cursor-pointer border border-dashed border-border hover:border-emerald-500/50 hover:bg-emerald-500/5 rounded-xl p-4 flex flex-col items-center justify-center gap-2 transition-all">
                <input
                  type="file"
                  className="hidden"
                  accept=".txt"
                  onChange={handleImportFile}
                />
                <Hash className="w-5 h-5 text-muted-foreground" />
                <span className="text-xs text-muted-foreground">日期列表.txt</span>
              </label>
            </div>
          </div>
        </div>

        {/* Right Panel: Visualization & Logs */}
        <div className="lg:col-span-8 flex flex-col gap-6">
          {/* Status Deck */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <StatusBadge
              label="进度"
              value={`${progressPercent}%`}
              active={isRunning}
            />
            <StatusBadge
              label="已尝试"
              value={progress?.total_attempted || 0}
            />
            <StatusBadge
              label="耗时"
              value={`${progress?.elapsed_seconds || 0}s`}
            />
            <StatusBadge
              label="状态"
              value={isRunning ? "进行中" : "空闲"}
              active={isRunning}
            />
          </div>

          {/* Visualizer */}
          <div className="relative bg-black rounded-2xl border border-border/50 p-6 overflow-hidden min-h-[200px] flex items-center justify-center">
            {/* Background Grid */}
            <div
              className="absolute inset-0 opacity-20"
              style={{
                backgroundImage:
                  "linear-gradient(rgba(255, 255, 255, 0.05) 1px, transparent 1px), linear-gradient(90deg, rgba(255, 255, 255, 0.05) 1px, transparent 1px)",
                backgroundSize: "20px 20px",
              }}
            />

            {/* Central Display */}
            <div className="relative z-10 text-center space-y-2">
              {progress?.found ? (
                <div className="animate-in zoom-in duration-300">
                  <CheckCircle2 className="w-16 h-16 text-green-500 mx-auto mb-2" />
                  <div className="text-2xl font-mono font-bold text-green-400">
                    已找到
                  </div>
                  <div className="text-white bg-green-500/20 px-4 py-1 rounded font-mono">
                    {progress.result}
                  </div>
                </div>
              ) : isRunning ? (
                <div className="space-y-4">
                  <div className="text-4xl font-mono font-bold text-primary animate-pulse tracking-widest">
                    {progress?.current_password || "初始化中..."}
                  </div>
                  <div className="w-64 h-2 bg-white/10 rounded-full mx-auto overflow-hidden">
                    <div
                      className="h-full bg-primary transition-all duration-100"
                      style={{ width: `${progressPercent}%` }}
                    />
                  </div>
                </div>
              ) : (
                <div className="text-muted-foreground flex flex-col items-center">
                  <Terminal className="w-12 h-12 mb-2 opacity-50" />
                  <span>准备开始任务</span>
                </div>
              )}
            </div>
          </div>

          {/* Terminal Output */}
          <TerminalLog logs={logs} />
        </div>
      </div>

      {/* History Section (Simple list for now) */}
      <div className="mt-8 border-t border-border/50 pt-8">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <History className="w-5 h-5" />
          最近记录
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {history.slice(0, 6).map((record) => (
            <div
              key={record.id}
              className="bg-card border border-border/50 p-4 rounded-xl flex items-center justify-between"
            >
              <div>
                <div className="font-mono font-bold">{record.username}</div>
                <div className="text-xs text-muted-foreground">
                  {new Date(record.created_at).toLocaleDateString()}
                </div>
              </div>
              <div className="flex items-center gap-2">
                <div className="text-sm font-mono bg-primary/10 text-primary px-2 py-1 rounded">
                  {record.password_date || "无"}
                </div>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-8 w-8 text-muted-foreground hover:text-primary"
                  onClick={() => handleEditRecord(record)}
                >
                  <Edit2 className="w-4 h-4" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-8 w-8 text-muted-foreground hover:text-destructive"
                  onClick={() => handleDeleteRecord(record)}
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </div>
            </div>
          ))}
          {history.length === 0 && (
            <div className="text-muted-foreground italic">暂无记录</div>
          )}
        </div>
      </div>

      {/* All Records Table */}
      <div className="mt-8 border-t border-border/50 pt-8">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <History className="w-5 h-5" />
          全部记录
        </h3>
        <div className="rounded-2xl border border-border/50 overflow-hidden bg-card/40 backdrop-blur-md shadow-sm">
          <div className="overflow-x-auto">
            <table className="w-full text-sm text-left">
              <thead className="bg-muted/50 text-muted-foreground font-medium border-b border-border/50">
                <tr>
                  <th className="px-6 py-4">学号</th>
                  <th className="px-6 py-4">姓名</th>
                  <th className="px-6 py-4">班级</th>
                  <th className="px-6 py-4">密码/日期</th>
                  <th className="px-6 py-4">创建时间</th>
                  <th className="px-6 py-4 text-right">操作</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border/30">
                {history.length === 0 ? (
                  <tr>
                    <td
                      colSpan={6}
                      className="px-6 py-12 text-center text-muted-foreground"
                    >
                      暂无记录
                    </td>
                  </tr>
                ) : (
                  history.map((record) => (
                    <tr key={record.id} className="hover:bg-muted/30">
                      <td className="px-6 py-4 font-mono">
                        {record.username}
                      </td>
                      <td className="px-6 py-4">{record.name || "-"}</td>
                      <td className="px-6 py-4">{record.class_name || "-"}</td>
                      <td className="px-6 py-4 font-mono">
                        {record.password_date || "-"}
                      </td>
                      <td className="px-6 py-4 text-muted-foreground">
                        {new Date(record.created_at).toLocaleString()}
                      </td>
                      <td className="px-6 py-4 text-right">
                        <div className="flex items-center justify-end gap-2">
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-8 w-8 text-muted-foreground hover:text-primary"
                            onClick={() => handleEditRecord(record)}
                          >
                            <Edit2 className="w-4 h-4" />
                          </Button>
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-8 w-8 text-muted-foreground hover:text-destructive"
                            onClick={() => handleDeleteRecord(record)}
                          >
                            <Trash2 className="w-4 h-4" />
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </ToolLayout>
  );
}
