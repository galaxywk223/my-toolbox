import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, Play, History, FileUp, Database, Search, Filter } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import * as XLSX from "xlsx";

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
  encoded_value?: string | null;
  year?: number | null;
  created_at: string;
  status: string;
}

interface StudentImport {
  username: string;
  name: string;
  class_name: string;
}

interface DateImport {
  username: string;
  password_date: string;
  encoded_value?: string | null;
}

export default function PasswordCrackerTool() {
  const navigate = useNavigate();
  const [username, setUsername] = useState("239074295");
  const [name, setName] = useState("");
  const [year, setYear] = useState(2005);
  const [concurrency, setConcurrency] = useState(10);
  const [isRunning, setIsRunning] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [importRows, setImportRows] = useState<StudentImport[]>([]);
  const [importMessage, setImportMessage] = useState("");
  const [isDateImporting, setIsDateImporting] = useState(false);
  const [dateImportRows, setDateImportRows] = useState<DateImport[]>([]);
  const [dateImportMessage, setDateImportMessage] = useState("");
  const [progress, setProgress] = useState<CrackProgress | null>(null);
  const [history, setHistory] = useState<PasswordResult[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const [message, setMessage] = useState("");
  const [filterText, setFilterText] = useState("");
  const [filterClass, setFilterClass] = useState("all");
  const [showWithDateOnly, setShowWithDateOnly] = useState(false);
  const [actionMessage, setActionMessage] = useState("");

  useEffect(() => {
    loadHistory();

    const unlistenPromise = listen<CrackProgress>("crack_progress", (event) => {
      setProgress(event.payload);
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const loadHistory = async () => {
    try {
      const result = await invoke<PasswordResult[]>("get_crack_history");
      setHistory(result);
    } catch (error) {
      console.error("Failed to load history:", error);
    }
  };

  const handleEditRecord = async (record: PasswordResult) => {
    const nameValue = window.prompt("姓名", record.name ?? "");
    if (nameValue === null) return;
    const classValue = window.prompt("班级", record.class_name ?? "");
    if (classValue === null) return;
    const passwordDate = window.prompt("日期/密码", record.password_date ?? "");
    if (passwordDate === null) return;
    try {
      await invoke("update_password_result", {
        username: record.username,
        name: nameValue.trim() ? nameValue.trim() : null,
        class_name: classValue.trim() ? classValue.trim() : null,
        password_date: passwordDate.trim() ? passwordDate.trim() : null,
      });
      await loadHistory();
      setActionMessage("已更新记录");
    } catch (error) {
      setActionMessage(`更新失败: ${error}`);
    }
  };

  const handleDeleteRecord = async (record: PasswordResult) => {
    const ok = window.confirm(`确定删除 ${record.username} 这条记录吗？`);
    if (!ok) return;
    try {
      await invoke("delete_password_result", { username: record.username });
      await loadHistory();
      setActionMessage("已删除记录");
    } catch (error) {
      setActionMessage(`删除失败: ${error}`);
    }
  };

  const handleCrack = async () => {
    if (!username.trim()) {
      setMessage("请输入用户名");
      return;
    }

    setIsRunning(true);
    setMessage("");
    setProgress(null);

    try {
      const result = await invoke<string>("crack_password", {
        request: {
          username: username.trim(),
          name: name.trim() ? name.trim() : null,
          year: parseInt(year.toString(), 10),
          concurrency: parseInt(concurrency.toString(), 10),
        },
      });
      setMessage(result);
      // 重新加载历史记录
      setTimeout(loadHistory, 500);
    } catch (error) {
      setMessage(`错误: ${error}`);
    } finally {
      setIsRunning(false);
    }
  };

  const parseStudentSheet = (rows: Record<string, unknown>[]) => {
    const requiredKeys = ["学号", "姓名", "班级"];
    const headerKeys = new Set<string>();
    rows.forEach((row) => Object.keys(row).forEach((k) => headerKeys.add(k)));
    const missing = requiredKeys.filter((k) => !headerKeys.has(k));
    if (missing.length > 0) {
      return {
        error: `缺少字段: ${missing.join("、")}`,
        data: [] as StudentImport[],
      };
    }

    const data = rows
      .map((row) => {
        const username = String(row["学号"] ?? "").trim();
        const nameValue = String(row["姓名"] ?? "").trim();
        const classValue = String(row["班级"] ?? "").trim();
        return {
          username,
          name: nameValue,
          class_name: classValue,
        };
      })
      .filter((row) => row.username.length > 0);

    if (data.length === 0) {
      return { error: "未解析到有效的学号数据", data: [] as StudentImport[] };
    }

    return { error: "", data };
  };

  const handleImportFile = async (file: File) => {
    setImportMessage("");
    setImportRows([]);

    try {
      const arrayBuffer = await file.arrayBuffer();
      const workbook = XLSX.read(arrayBuffer, { type: "array" });
      const firstSheetName = workbook.SheetNames[0];
      if (!firstSheetName) {
        setImportMessage("未找到工作表");
        return;
      }
      const sheet = workbook.Sheets[firstSheetName];
      const rows = XLSX.utils.sheet_to_json<Record<string, unknown>>(sheet, {
        defval: "",
      });
      const parsed = parseStudentSheet(rows);
      if (parsed.error) {
        setImportMessage(parsed.error);
        return;
      }
      setImportRows(parsed.data);
      setImportMessage(`已解析 ${parsed.data.length} 条记录`);
    } catch (error) {
      setImportMessage(`解析失败: ${String(error)}`);
    }
  };

  const handleImport = async () => {
    if (importRows.length === 0) {
      setImportMessage("请先选择并解析 Excel 文件");
      return;
    }
    setIsImporting(true);
    try {
      const result = await invoke<{ inserted: number; updated: number }>(
        "import_students",
        { students: importRows }
      );
      setImportMessage(
        `导入完成：新增 ${result.inserted} 条，更新 ${result.updated} 条`
      );
    } catch (error) {
      setImportMessage(`导入失败: ${error}`);
    } finally {
      setIsImporting(false);
    }
  };

  const parseDateText = (text: string) => {
    const lines = text.split(/\r?\n/);
    const results: DateImport[] = [];
    let current: Partial<DateImport> = {};

    const pushIfValid = () => {
      if (current.username && current.password_date) {
        results.push({
          username: current.username,
          password_date: current.password_date,
          encoded_value: current.encoded_value ?? null,
        });
      }
    };

    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line) continue;
      if (line.startsWith("Username:")) {
        pushIfValid();
        current = {
          username: line.replace("Username:", "").trim(),
        };
      } else if (line.startsWith("Plain Password (Date):")) {
        current.password_date = line
          .replace("Plain Password (Date):", "")
          .trim();
      } else if (line.startsWith("Value for 'encoded' field Sent:")) {
        current.encoded_value = line
          .replace("Value for 'encoded' field Sent:", "")
          .trim();
      }
    }
    pushIfValid();

    const deduped = new Map<string, DateImport>();
    for (const item of results) {
      if (item.username && item.password_date) {
        deduped.set(item.username, item);
      }
    }
    return Array.from(deduped.values());
  };

  const handleImportDateFile = async (file: File) => {
    setDateImportMessage("");
    setDateImportRows([]);
    try {
      const text = await file.text();
      const parsed = parseDateText(text);
      if (parsed.length === 0) {
        setDateImportMessage("未解析到有效的日期记录");
        return;
      }
      setDateImportRows(parsed);
      setDateImportMessage(`已解析 ${parsed.length} 条日期记录`);
    } catch (error) {
      setDateImportMessage(`解析失败: ${String(error)}`);
    }
  };

  const handleImportDates = async () => {
    if (dateImportRows.length === 0) {
      setDateImportMessage("请先选择并解析 TXT 文件");
      return;
    }
    setIsDateImporting(true);
    try {
      const result = await invoke<{ inserted: number; updated: number }>(
        "import_dates",
        { dates: dateImportRows }
      );
      setDateImportMessage(
        `导入完成：新增 ${result.inserted} 条，更新 ${result.updated} 条`
      );
    } catch (error) {
      setDateImportMessage(`导入失败: ${error}`);
    } finally {
      setIsDateImporting(false);
    }
  };

  const progressPercent = progress
    ? Math.round((progress.total_attempted / progress.total_passwords) * 100)
    : 0;

  const classOptions = Array.from(
    new Set(
      history
        .map((record) => record.class_name)
        .filter((value): value is string => Boolean(value && value.trim()))
    )
  ).sort((a, b) => a.localeCompare(b));

  const filteredRecords = history
    .filter((record) => {
      if (showWithDateOnly && !record.password_date) return false;
      if (filterClass !== "all" && record.class_name !== filterClass) return false;
      if (!filterText.trim()) return true;
      const keyword = filterText.trim().toLowerCase();
      return (
        record.username.toLowerCase().includes(keyword) ||
        (record.name ?? "").toLowerCase().includes(keyword) ||
        (record.class_name ?? "").toLowerCase().includes(keyword) ||
        (record.password_date ?? "").toLowerCase().includes(keyword)
      );
    })
    .sort((a, b) => {
      const aHasDate = Boolean(a.password_date);
      const bHasDate = Boolean(b.password_date);
      if (aHasDate !== bHasDate) {
        return aHasDate ? -1 : 1;
      }
      const aNum = Number.parseInt(a.username, 10);
      const bNum = Number.parseInt(b.username, 10);
      if (!Number.isNaN(aNum) && !Number.isNaN(bNum)) {
        return aNum - bNum;
      }
      return a.username.localeCompare(b.username);
    });

  return (
    <div className="space-y-6">
      <Button
        variant="ghost"
        className="gap-2 pl-0 hover:bg-transparent text-muted-foreground hover:text-foreground"
        onClick={() => navigate("/")}
      >
        <ArrowLeft className="w-4 h-4" />
        返回首页
      </Button>

      <div className="space-y-2">
        <h1 className="text-3xl font-bold tracking-tight text-foreground flex items-center gap-3">
            <span className="p-2 bg-primary/10 text-primary rounded-lg">
                <Database className="w-8 h-8" />
            </span>
          教务日期查询
        </h1>
        <p className="text-muted-foreground">
          基于日期格式的教务日期查询（仅供校内调试使用）
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* 配置卡片 */}
        <Card className="lg:col-span-2 shadow-lg shadow-primary/5">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
                <Filter className="w-5 h-5 text-primary" />
                配置参数
            </CardTitle>
            <CardDescription>设置查询的基本参数</CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                {/* 用户名输入 */}
                <div className="space-y-2">
                  <label className="text-sm font-medium text-foreground">
                    用户名 / 学号
                  </label>
                  <input
                    type="text"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    disabled={isRunning}
                    placeholder="输入用户名或学号"
                    className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                  />
                </div>

                {/* 姓名输入 */}
                <div className="space-y-2">
                  <label className="text-sm font-medium text-foreground">
                    姓名（可选）
                  </label>
                  <input
                    type="text"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    disabled={isRunning}
                    placeholder="输入姓名（可留空）"
                    className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                  />
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                 {/* 年份输入 */}
                <div className="space-y-2">
                  <label className="text-sm font-medium text-foreground">
                    年份
                  </label>
                  <input
                    type="number"
                    value={year}
                    onChange={(e) => setYear(parseInt(e.target.value))}
                    disabled={isRunning}
                    min="1900"
                    max="2100"
                    className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                  />
                </div>

                {/* 并发数 */}
                <div className="space-y-2">
                  <label className="text-sm font-medium text-foreground">
                    并发数
                  </label>
                  <input
                    type="number"
                    value={concurrency}
                    onChange={(e) => setConcurrency(parseInt(e.target.value))}
                    disabled={isRunning}
                    min="1"
                    max="50"
                    className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                  />
                </div>
            </div>

            {/* 开始按钮 */}
            <Button
              onClick={handleCrack}
              disabled={isRunning}
              className="w-full gap-2 bg-primary hover:bg-primary/90 text-primary-foreground shadow-lg shadow-primary/20 h-11 text-lg font-semibold"
            >
              <Play className={`w-5 h-5 ${isRunning ? 'animate-spin' : ''}`} />
              {isRunning ? "查询中..." : "开始查询"}
            </Button>
          </CardContent>
        </Card>

        {/* 信息卡片 */}
        <Card className="h-fit">
          <CardHeader>
            <CardTitle>实时状态</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {progress ? (
              <div className="space-y-4 animate-in fade-in slide-in-from-bottom-2">
                <div>
                  <div className="flex justify-between text-xs text-muted-foreground mb-1">
                    <span>进度</span>
                    <span>{progressPercent}%</span>
                  </div>
                  <div className="w-full bg-secondary/50 rounded-full h-2 overflow-hidden">
                    <div
                      className="bg-primary h-2 rounded-full transition-all duration-300 ease-out shadow-[0_0_10px_rgba(124,58,237,0.5)]"
                      style={{ width: `${progressPercent}%` }}
                    ></div>
                  </div>
                  <p className="text-xs text-muted-foreground mt-1 text-center font-mono">
                    {progress.total_attempted} / {progress.total_passwords}
                  </p>
                </div>
                <div className="bg-muted/50 p-3 rounded-lg space-y-2">
                    <div className="flex justify-between text-xs">
                        <span className="text-muted-foreground">当前尝试</span>
                        <span className="font-mono text-foreground">{progress.current_password}</span>
                    </div>
                     <div className="flex justify-between text-xs">
                        <span className="text-muted-foreground">耗时</span>
                        <span className="font-mono text-foreground">{progress.elapsed_seconds}s</span>
                    </div>
                </div>
                
                {progress.found && (
                  <div className="p-3 bg-green-500/10 rounded-lg border border-green-500/20 animate-bounce">
                    <p className="text-sm font-bold text-green-600 dark:text-green-400 text-center flex items-center justify-center gap-2">
                      <span className="inline-block w-2 h-2 rounded-full bg-green-500"></span>
                      已找到: {progress.result}
                    </p>
                  </div>
                )}
              </div>
            ) : (
                <div className="py-8 text-center text-muted-foreground flex flex-col items-center gap-2">
                    <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center">
                         <Play className="w-6 h-6 text-muted-foreground/50" />
                    </div>
                    <p>等待任务开始...</p>
                </div>
            )}

            {message && (
              <div
                className={`p-3 rounded-lg border ${
                  message.includes("错误")
                    ? "bg-destructive/10 border-destructive/20 text-destructive"
                    : "bg-primary/10 border-primary/20 text-primary"
                }`}
              >
                <p className="text-sm font-medium">{message}</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

        {/* 导入区域 */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* 导入学生信息 */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <FileUp className="w-5 h-5 text-blue-500" />
                    导入学生信息
                </CardTitle>
                <CardDescription>Excel文件 (学号、姓名、班级)</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="relative group">
                    <input
                      type="file"
                      accept=".xlsx,.xls"
                      onChange={(e) => {
                        const file = e.target.files?.[0];
                        if (file) handleImportFile(file);
                      }}
                      className="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-10"
                    />
                    <div className="border-2 border-dashed border-muted-foreground/25 rounded-lg p-6 text-center transition-colors group-hover:border-primary/50 group-hover:bg-primary/5">
                         <FileUp className="w-8 h-8 mx-auto text-muted-foreground mb-2 group-hover:text-primary transition-colors" />
                         <p className="text-sm text-muted-foreground group-hover:text-primary transition-colors">点击或拖拽上传 Excel 文件</p>
                    </div>
                </div>
                
                {importMessage && (
                  <p className="text-xs text-muted-foreground text-center bg-muted/50 p-2 rounded">
                    {importMessage}
                  </p>
                )}
                <div className="flex items-center justify-between">
                  <span className="text-xs text-muted-foreground bg-muted px-2 py-1 rounded">
                    待导入: {importRows.length}
                  </span>
                  <Button
                    size="sm"
                    onClick={handleImport}
                    disabled={isImporting || importRows.length === 0}
                    className="bg-blue-600 hover:bg-blue-700 text-white"
                  >
                    {isImporting ? "导入中..." : "确认导入"}
                  </Button>
                </div>
              </CardContent>
            </Card>
    
            {/* 导入日期记录 */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <FileUp className="w-5 h-5 text-emerald-500" />
                    导入日期记录
                </CardTitle>
                <CardDescription>
                  TXT 日志文件导入
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                 <div className="relative group">
                    <input
                      type="file"
                      accept=".txt"
                      onChange={(e) => {
                        const file = e.target.files?.[0];
                        if (file) handleImportDateFile(file);
                      }}
                      className="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-10"
                    />
                    <div className="border-2 border-dashed border-muted-foreground/25 rounded-lg p-6 text-center transition-colors group-hover:border-emerald-500/50 group-hover:bg-emerald-500/5">
                         <FileUp className="w-8 h-8 mx-auto text-muted-foreground mb-2 group-hover:text-emerald-500 transition-colors" />
                         <p className="text-sm text-muted-foreground group-hover:text-emerald-500 transition-colors">点击或拖拽上传 TXT 文件</p>
                    </div>
                </div>

                {dateImportMessage && (
                  <p className="text-xs text-muted-foreground text-center bg-muted/50 p-2 rounded">
                    {dateImportMessage}
                  </p>
                )}
                <div className="flex items-center justify-between">
                  <span className="text-xs text-muted-foreground bg-muted px-2 py-1 rounded">
                    待导入: {dateImportRows.length}
                  </span>
                  <Button
                    size="sm"
                    onClick={handleImportDates}
                    disabled={isDateImporting || dateImportRows.length === 0}
                    className="bg-emerald-600 hover:bg-emerald-700 text-white"
                  >
                    {isDateImporting ? "导入中..." : "确认导入"}
                  </Button>
                </div>
              </CardContent>
            </Card>
        </div>

      {/* 历史记录 */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <div className="space-y-1">
            <CardTitle className="flex items-center gap-2">
                 <History className="w-5 h-5 text-primary" />
                历史记录
            </CardTitle>
            <CardDescription>之前的查询成功记录</CardDescription>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              setShowHistory(!showHistory);
              if (!showHistory) loadHistory();
            }}
            className="gap-2"
          >
            <History className="w-4 h-4" />
            {showHistory ? "收起" : "展开"}
          </Button>
        </CardHeader>

        {showHistory && (
          <CardContent>
            {history.length === 0 ? (
              <div className="text-center py-8 text-muted-foreground">
                  暂无历史记录
              </div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 animate-in fade-in slide-in-from-bottom-4">
                {history.map((record) => (
                  <div
                    key={record.id}
                    className="p-4 rounded-xl bg-muted/30 border border-border/50 hover:bg-muted/50 transition-colors"
                  >
                    <div className="flex justify-between items-start mb-2">
                       <span className="font-mono font-bold text-lg text-primary">{record.username}</span>
                       <span className="text-xs text-muted-foreground">{new Date(record.created_at).toLocaleDateString()}</span>
                    </div>
                    <div className="space-y-1 text-sm">
                        {record.name && (
                             <div className="flex justify-between">
                                <span className="text-muted-foreground">姓名:</span>
                                <span>{record.name}</span>
                             </div>
                        )}
                        {record.class_name && (
                             <div className="flex justify-between">
                                <span className="text-muted-foreground">班级:</span>
                                <span>{record.class_name}</span>
                             </div>
                        )}
                        <div className="flex justify-between font-medium bg-primary/10 p-1 rounded px-2 mt-2">
                            <span className="text-primary-foreground/80 dark:text-primary">日期:</span>
                            <span className="text-primary">{record.password_date ?? "未填写"}</span>
                        </div>
                        <div className="flex items-center gap-2 pt-2">
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleEditRecord(record)}
                          >
                            编辑
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleDeleteRecord(record)}
                          >
                            删除
                          </Button>
                        </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        )}
      </Card>

      {/* 可视化列表 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Search className="w-5 h-5 text-primary" />
            记录查询
          </CardTitle>
          <CardDescription>显示学号、姓名、班级和日期，可查询与筛选</CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-12 gap-4">
            <div className="md:col-span-5">
                 <div className="relative">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                    <input
                      type="text"
                      value={filterText}
                      onChange={(e) => setFilterText(e.target.value)}
                      placeholder="搜索学号/姓名/班级/日期..."
                      className="w-full pl-9 pr-4 py-2 border border-input rounded-md bg-background text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary transition-all"
                    />
                 </div>
            </div>
            <div className="md:col-span-4">
                 <select
                    value={filterClass}
                    onChange={(e) => setFilterClass(e.target.value)}
                    className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground focus:outline-none focus:ring-2 focus:ring-primary transition-all"
                  >
                    <option value="all">全部班级</option>
                    {classOptions.map((cls) => (
                      <option key={cls} value={cls}>
                        {cls}
                      </option>
                    ))}
                  </select>
            </div>
            <div className="md:col-span-3 flex items-center">
                 <label className="flex items-center gap-2 text-sm text-foreground cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={showWithDateOnly}
                      onChange={(e) => setShowWithDateOnly(e.target.checked)}
                      className="w-4 h-4 rounded border-input text-primary focus:ring-primary accent-primary"
                    />
                    仅显示有日期
                  </label>
            </div>
          </div>

          {actionMessage && (
            <div className="p-3 rounded-lg border bg-muted/30 text-sm">
              {actionMessage}
            </div>
          )}

          {filteredRecords.length === 0 ? (
             <div className="text-center py-12 text-muted-foreground bg-muted/10 rounded-xl border border-dashed border-border">
                <Search className="w-8 h-8 mx-auto mb-2 opacity-50" />
                没有符合条件的记录
             </div>
          ) : (
            <div className="rounded-xl border border-border overflow-hidden">
              <table className="w-full text-sm">
                <thead className="bg-muted/50">
                  <tr className="text-left text-muted-foreground">
                    <th className="py-3 px-4 font-medium">学号</th>
                    <th className="py-3 px-4 font-medium">姓名</th>
                    <th className="py-3 px-4 font-medium">班级</th>
                    <th className="py-3 px-4 font-medium">日期</th>
                    <th className="py-3 px-4 font-medium">操作</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border/50">
                  {filteredRecords.map((record) => (
                    <tr
                      key={record.id}
                      className="hover:bg-muted/30 transition-colors"
                    >
                      <td className="py-3 px-4 font-mono">{record.username}</td>
                      <td className="py-3 px-4">{record.name ?? "—"}</td>
                      <td className="py-3 px-4">{record.class_name ?? "—"}</td>
                      <td className="py-3 px-4">
                          {record.password_date ? (
                               <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400">
                                   {record.password_date}
                               </span>
                          ) : (
                              <span className="text-muted-foreground">—</span>
                          )}
                      </td>
                      <td className="py-3 px-4">
                        <div className="flex items-center gap-2">
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleEditRecord(record)}
                          >
                            编辑
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleDeleteRecord(record)}
                          >
                            删除
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
