import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, Play, History } from "lucide-react";
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
    <div className="min-h-screen bg-slate-50 dark:bg-slate-950 p-8">
      <div className="max-w-4xl mx-auto space-y-8">
        <Button
          variant="ghost"
          className="gap-2 pl-0 hover:bg-transparent hover:text-slate-900 dark:hover:text-slate-50"
          onClick={() => navigate("/")}
        >
          <ArrowLeft className="w-4 h-4" />
          返回首页
        </Button>

        <div className="space-y-2">
          <h1 className="text-3xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
            教务日期查询
          </h1>
          <p className="text-slate-500 dark:text-slate-400">
            基于日期格式的教务日期查询（仅供校内调试使用）
          </p>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* 配置卡片 */}
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>配置参数</CardTitle>
              <CardDescription>设置查询的基本参数</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              {/* 用户名输入 */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-900 dark:text-slate-50">
                  用户名
                </label>
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  disabled={isRunning}
                  placeholder="输入用户名或学号"
                  className="w-full px-3 py-2 border border-slate-300 dark:border-slate-600 rounded-md bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-50 placeholder-slate-400 dark:placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                />
              </div>

              {/* 姓名输入 */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-900 dark:text-slate-50">
                  姓名（可选）
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  disabled={isRunning}
                  placeholder="输入姓名（可留空）"
                  className="w-full px-3 py-2 border border-slate-300 dark:border-slate-600 rounded-md bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-50 placeholder-slate-400 dark:placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                />
              </div>

              {/* 年份输入 */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-900 dark:text-slate-50">
                  年份
                </label>
                <input
                  type="number"
                  value={year}
                  onChange={(e) => setYear(parseInt(e.target.value))}
                  disabled={isRunning}
                  min="1900"
                  max="2100"
                  className="w-full px-3 py-2 border border-slate-300 dark:border-slate-600 rounded-md bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-50 placeholder-slate-400 dark:placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                />
              </div>

              {/* 并发数 */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-900 dark:text-slate-50">
                  并发数
                </label>
                <input
                  type="number"
                  value={concurrency}
                  onChange={(e) => setConcurrency(parseInt(e.target.value))}
                  disabled={isRunning}
                  min="1"
                  max="50"
                  className="w-full px-3 py-2 border border-slate-300 dark:border-slate-600 rounded-md bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-50 placeholder-slate-400 dark:placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                />
              </div>

              {/* 开始按钮 */}
              <Button
                onClick={handleCrack}
                disabled={isRunning}
                className="w-full gap-2 bg-blue-500 hover:bg-blue-600 text-white"
              >
                <Play className="w-4 h-4" />
                {isRunning ? "查询中..." : "开始查询"}
              </Button>
            </CardContent>
          </Card>

          {/* 信息卡片 */}
          <Card>
            <CardHeader>
              <CardTitle>信息</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {progress && (
                <div className="space-y-3">
                  <div>
                    <p className="text-xs text-slate-500 dark:text-slate-400 mb-1">
                      进度
                    </p>
                    <div className="w-full bg-slate-200 dark:bg-slate-700 rounded-full h-2">
                      <div
                        className="bg-blue-500 h-2 rounded-full transition-all"
                        style={{ width: `${progressPercent}%` }}
                      ></div>
                    </div>
                    <p className="text-xs text-slate-600 dark:text-slate-400 mt-1">
                      {progress.total_attempted} / {progress.total_passwords}
                    </p>
                  </div>
                  <div>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      当前尝试日期: {progress.current_password}
                    </p>
                  </div>
                  <div>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      耗时: {progress.elapsed_seconds}秒
                    </p>
                  </div>
                  {progress.found && (
                    <div className="p-3 bg-green-50 dark:bg-green-900/20 rounded border border-green-200 dark:border-green-800">
                      <p className="text-sm font-medium text-green-700 dark:text-green-300">
                        ✓ 已找到日期: {progress.result}
                      </p>
                    </div>
                  )}
                </div>
              )}

              {message && (
                <div
                  className={`p-3 rounded ${
                    message.includes("错误")
                      ? "bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300"
                      : "bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300"
                  }`}
                >
                  <p className="text-sm">{message}</p>
                </div>
              )}
            </CardContent>
          </Card>
        </div>

        {/* 导入学生信息 */}
        <Card>
          <CardHeader>
            <CardTitle>导入学生信息</CardTitle>
            <CardDescription>仅导入学号、姓名、班级（不含日期）</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <input
              type="file"
              accept=".xlsx,.xls"
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (file) handleImportFile(file);
              }}
              className="block w-full text-sm text-slate-600 file:mr-4 file:rounded-md file:border-0 file:bg-slate-100 file:px-4 file:py-2 file:text-sm file:font-medium file:text-slate-700 hover:file:bg-slate-200 dark:text-slate-400 dark:file:bg-slate-800 dark:file:text-slate-200 dark:hover:file:bg-slate-700"
            />
            {importMessage && (
              <p className="text-sm text-slate-600 dark:text-slate-400">
                {importMessage}
              </p>
            )}
            <div className="flex items-center justify-between">
              <p className="text-xs text-slate-500 dark:text-slate-500">
                当前待导入：{importRows.length} 条
              </p>
              <Button
                onClick={handleImport}
                disabled={isImporting || importRows.length === 0}
                className="bg-emerald-500 hover:bg-emerald-600 text-white"
              >
                {isImporting ? "导入中..." : "开始导入"}
              </Button>
            </div>
          </CardContent>
        </Card>

        {/* 导入日期记录 */}
        <Card>
          <CardHeader>
            <CardTitle>导入日期记录</CardTitle>
            <CardDescription>
              支持两种 TXT 格式：以 “Username:” 开头或带 “--- Log entry at ... ---” 分隔的日志格式
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <input
              type="file"
              accept=".txt"
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (file) handleImportDateFile(file);
              }}
              className="block w-full text-sm text-slate-600 file:mr-4 file:rounded-md file:border-0 file:bg-slate-100 file:px-4 file:py-2 file:text-sm file:font-medium file:text-slate-700 hover:file:bg-slate-200 dark:text-slate-400 dark:file:bg-slate-800 dark:file:text-slate-200 dark:hover:file:bg-slate-700"
            />
            {dateImportMessage && (
              <p className="text-sm text-slate-600 dark:text-slate-400">
                {dateImportMessage}
              </p>
            )}
            <div className="flex items-center justify-between">
              <p className="text-xs text-slate-500 dark:text-slate-500">
                当前待导入：{dateImportRows.length} 条
              </p>
              <Button
                onClick={handleImportDates}
                disabled={isDateImporting || dateImportRows.length === 0}
                className="bg-emerald-500 hover:bg-emerald-600 text-white"
              >
                {isDateImporting ? "导入中..." : "开始导入"}
              </Button>
            </div>
          </CardContent>
        </Card>

        {/* 历史记录 */}
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <div>
              <CardTitle>历史记录</CardTitle>
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
              {showHistory ? "隐藏" : "查看"}
            </Button>
          </CardHeader>

          {showHistory && (
            <CardContent>
              {history.length === 0 ? (
                <p className="text-slate-500 dark:text-slate-400 text-sm">
                  暂无记录
                </p>
              ) : (
                <div className="space-y-3">
                  {history.map((record) => (
                    <div
                      key={record.id}
                      className="p-3 border border-slate-200 dark:border-slate-700 rounded-md bg-slate-50 dark:bg-slate-900"
                    >
                      <div className="flex justify-between items-start">
                        <div className="space-y-1 flex-1">
                          <p className="text-sm font-medium text-slate-900 dark:text-slate-50">
                            用户: {record.username}
                          </p>
                          {record.name && (
                            <p className="text-sm text-slate-600 dark:text-slate-400">
                              姓名: {record.name}
                            </p>
                          )}
                          {record.class_name && (
                            <p className="text-sm text-slate-600 dark:text-slate-400">
                              班级: {record.class_name}
                            </p>
                          )}
                          <p className="text-sm text-slate-600 dark:text-slate-400">
                            日期: {record.password_date ?? "未填写"}
                          </p>
                          <p className="text-xs text-slate-500 dark:text-slate-500">
                            年份: {record.year}
                          </p>
                        </div>
                        <p className="text-xs text-slate-400 dark:text-slate-600 whitespace-nowrap ml-4">
                          {new Date(record.created_at).toLocaleString("zh-CN")}
                        </p>
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
            <CardTitle>记录列表</CardTitle>
            <CardDescription>显示学号、姓名、班级和日期，可查询与筛选</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              <input
                type="text"
                value={filterText}
                onChange={(e) => setFilterText(e.target.value)}
                placeholder="搜索学号/姓名/班级/日期"
                className="w-full px-3 py-2 border border-slate-300 dark:border-slate-600 rounded-md bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-50 placeholder-slate-400 dark:placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              <select
                value={filterClass}
                onChange={(e) => setFilterClass(e.target.value)}
                className="w-full px-3 py-2 border border-slate-300 dark:border-slate-600 rounded-md bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-50 focus:outline-none focus:ring-2 focus:ring-blue-500"
              >
                <option value="all">全部班级</option>
                {classOptions.map((cls) => (
                  <option key={cls} value={cls}>
                    {cls}
                  </option>
                ))}
              </select>
              <label className="flex items-center gap-2 text-sm text-slate-600 dark:text-slate-400">
                <input
                  type="checkbox"
                  checked={showWithDateOnly}
                  onChange={(e) => setShowWithDateOnly(e.target.checked)}
                  className="h-4 w-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500"
                />
                仅显示有日期
              </label>
            </div>

            {filteredRecords.length === 0 ? (
              <p className="text-slate-500 dark:text-slate-400 text-sm">
                没有符合条件的记录
              </p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="text-left text-slate-500 dark:text-slate-400 border-b border-slate-200 dark:border-slate-700">
                      <th className="py-2 pr-4">学号</th>
                      <th className="py-2 pr-4">姓名</th>
                      <th className="py-2 pr-4">班级</th>
                      <th className="py-2 pr-4">日期</th>
                    </tr>
                  </thead>
                  <tbody>
                    {filteredRecords.map((record) => (
                      <tr
                        key={record.id}
                        className="border-b border-slate-100 dark:border-slate-800"
                      >
                        <td className="py-2 pr-4 text-slate-900 dark:text-slate-50">
                          {record.username}
                        </td>
                        <td className="py-2 pr-4 text-slate-700 dark:text-slate-300">
                          {record.name ?? "—"}
                        </td>
                        <td className="py-2 pr-4 text-slate-700 dark:text-slate-300">
                          {record.class_name ?? "—"}
                        </td>
                        <td className="py-2 pr-4 text-slate-700 dark:text-slate-300">
                          {record.password_date ?? "—"}
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
    </div>
  );
}
