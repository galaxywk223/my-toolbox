import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Calendar,
  RefreshCw,
  Upload,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ToolLayout } from "@/components/ToolLayout";

interface ScheduleTerm {
  term: string;
  updated_at: string;
}

interface ScheduleEntry {
  id: number;
  term: string;
  weekday: number;
  period_label: string;
  period_index?: number | null;
  course_name: string;
  teacher?: string | null;
  location?: string | null;
  week_text?: string | null;
  week_numbers: number[];
  updated_at: string;
}

const weekdays = [
  { id: 1, name: "星期一" },
  { id: 2, name: "星期二" },
  { id: 3, name: "星期三" },
  { id: 4, name: "星期四" },
  { id: 5, name: "星期五" },
  { id: 6, name: "星期六" },
  { id: 7, name: "星期日" },
];

export default function ScheduleTool() {
  const [terms, setTerms] = useState<ScheduleTerm[]>([]);
  const [entries, setEntries] = useState<ScheduleEntry[]>([]);
  const [selectedTerm, setSelectedTerm] = useState("");
  const [selectedWeek, setSelectedWeek] = useState<number>(0);
  const [isSyncing, setIsSyncing] = useState(false);
  const [showSyncModal, setShowSyncModal] = useState(false);

  const [syncUsername, setSyncUsername] = useState("");
  const [syncPassword, setSyncPassword] = useState("");
  const [syncTerm, setSyncTerm] = useState("");
  const [syncMessage, setSyncMessage] = useState("");

  const loadTerms = async () => {
    try {
      const result = await invoke<ScheduleTerm[]>("get_schedule_terms");
      setTerms(result);
      if (result.length > 0) {
        setSelectedTerm((prev) => prev || result[0].term);
      } else {
        setSelectedTerm("");
      }
    } catch (error) {
      console.error("Failed to load terms:", error);
    }
  };

  const loadEntries = async (term: string) => {
    if (!term) {
      setEntries([]);
      return;
    }
    try {
      const result = await invoke<ScheduleEntry[]>("get_schedule_entries", {
        term,
      });
      setEntries(result);
    } catch (error) {
      console.error("Failed to load entries:", error);
    }
  };

  useEffect(() => {
    loadTerms();
  }, []);

  useEffect(() => {
    if (selectedTerm) {
      loadEntries(selectedTerm);
    } else {
      setEntries([]);
    }
  }, [selectedTerm]);

  const weekOptions = useMemo(() => {
    let maxWeek = 0;
    for (const entry of entries) {
      for (const week of entry.week_numbers) {
        if (week > maxWeek) maxWeek = week;
      }
    }
    if (maxWeek === 0) {
      maxWeek = 30;
    }
    return Array.from({ length: maxWeek }, (_, i) => i + 1);
  }, [entries]);

  const filteredEntries = useMemo(() => {
    if (!selectedWeek) return entries;
    return entries.filter(
      (entry) =>
        entry.week_numbers.length === 0 ||
        entry.week_numbers.includes(selectedWeek),
    );
  }, [entries, selectedWeek]);

  const periodRows = useMemo(() => {
    const map = new Map<string, { label: string; index: number }>();
    for (const entry of entries) {
      const index = entry.period_index ?? 999;
      if (!map.has(entry.period_label)) {
        map.set(entry.period_label, { label: entry.period_label, index });
      }
    }
    return Array.from(map.values()).sort((a, b) => a.index - b.index);
  }, [entries]);

  const handleSync = async () => {
    if (!syncUsername.trim() || !syncPassword.trim()) {
      setSyncMessage("请输入账号和密码");
      return;
    }
    setIsSyncing(true);
    setSyncMessage("");
    try {
      const term = syncTerm.trim() || selectedTerm || undefined;
      await invoke("sync_schedule", {
        request: {
          username: syncUsername.trim(),
          password: syncPassword,
          term,
        },
      });
      setSyncMessage("同步成功");
      await loadTerms();
      if (term) {
        setSelectedTerm(term);
        await loadEntries(term);
      }
      setSyncPassword("");
      setTimeout(() => setShowSyncModal(false), 1200);
    } catch (error) {
      setSyncMessage(`同步失败：${error}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const shiftWeek = (delta: number) => {
    if (selectedWeek === 0) {
      setSelectedWeek(1);
      return;
    }
    const next = selectedWeek + delta;
    if (next < 1) return;
    if (next > weekOptions.length) return;
    setSelectedWeek(next);
  };

  const classColors = [
    "text-emerald-600",
    "text-blue-600",
    "text-fuchsia-600",
    "text-amber-600",
    "text-cyan-600",
    "text-rose-600",
    "text-indigo-600",
    "text-teal-600",
    "text-orange-600",
    "text-violet-600",
    "text-lime-600",
    "text-sky-600",
  ];

  const hashCourse = (name: string) => {
    let hash = 0;
    for (let i = 0; i < name.length; i += 1) {
      hash = (hash * 31 + name.charCodeAt(i)) % 100000;
    }
    return hash;
  };

  const courseColorClass = (name: string) => {
    const index = Math.abs(hashCourse(name)) % classColors.length;
    return classColors[index];
  };

  return (
    <ToolLayout
      title="课程表"
      description="按学期与周次查看课程安排，支持同步更新与本地保存。"
      actions={
        <div className="flex items-center gap-2">
          {selectedTerm && (
            <div className="flex items-center gap-2 rounded-full border border-border px-3 py-2 text-xs text-muted-foreground">
              <Calendar className="w-3.5 h-3.5" />
              <span>{selectedTerm}</span>
            </div>
          )}
          <Button onClick={() => setShowSyncModal(true)} className="gap-2">
            <Upload className="w-4 h-4" />
            同步课表
          </Button>
          <Button
            variant="outline"
            className="gap-2"
            onClick={() => loadEntries(selectedTerm)}
            disabled={!selectedTerm}
          >
            <RefreshCw className="w-4 h-4" />
            刷新
          </Button>
        </div>
      }
    >
      <div className="space-y-6">
        <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between bg-card/30 p-4 rounded-2xl border border-border/50">
          <div className="flex items-center gap-3">
            <span className="text-sm text-muted-foreground">学期</span>
            <select
              className="h-9 rounded-lg border border-border bg-transparent px-3 text-sm outline-none"
              value={selectedTerm}
              onChange={(e) => setSelectedTerm(e.target.value)}
            >
              {terms.length === 0 && <option value="">暂无学期</option>}
              {terms.map((term) => (
                <option key={term.term} value={term.term}>
                  {term.term}
                </option>
              ))}
            </select>
          </div>

          <div className="flex items-center gap-3">
            <span className="text-sm text-muted-foreground">周次</span>
            <Button
              variant="outline"
              size="icon"
              onClick={() => shiftWeek(-1)}
              disabled={weekOptions.length === 0}
            >
              <ChevronLeft className="w-4 h-4" />
            </Button>
            <select
              className="h-9 rounded-lg border border-border bg-transparent px-3 text-sm outline-none"
              value={selectedWeek}
              onChange={(e) => setSelectedWeek(parseInt(e.target.value))}
            >
              <option value={0}>全部周次</option>
              {weekOptions.map((week) => (
                <option key={week} value={week}>
                  第{week}周
                </option>
              ))}
            </select>
            <Button
              variant="outline"
              size="icon"
              onClick={() => shiftWeek(1)}
              disabled={weekOptions.length === 0}
            >
              <ChevronRight className="w-4 h-4" />
            </Button>
          </div>
        </div>

        {selectedTerm ? (
          <div className="overflow-x-auto rounded-2xl border border-border/50 bg-card/40 backdrop-blur-md shadow-sm">
            <table className="w-full text-sm text-left">
              <thead className="bg-muted/50 text-muted-foreground font-medium border-b border-border/50">
                <tr>
                  <th className="px-4 py-3 w-28">节次</th>
                  {weekdays.map((day) => (
                    <th key={day.id} className="px-4 py-3 text-center">
                      {day.name}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-border/30">
                {periodRows.length === 0 ? (
                  <tr>
                    <td
                      colSpan={weekdays.length + 1}
                      className="px-6 py-12 text-center text-muted-foreground"
                    >
                      暂无课表数据
                    </td>
                  </tr>
                ) : (
                  periodRows.map((period) => (
                    <tr key={period.label}>
                      <td className="px-4 py-4 font-medium text-muted-foreground">
                        {period.label}
                      </td>
                      {weekdays.map((day) => {
                        const cellEntries = filteredEntries.filter(
                          (entry) =>
                            entry.weekday === day.id &&
                            entry.period_label === period.label,
                        );
                        return (
                          <td key={day.id} className="px-3 py-3 align-top">
                            <div className="space-y-2">
                              {cellEntries.length === 0 && (
                                <div className="text-xs text-muted-foreground">
                                  -
                                </div>
                              )}
                              {cellEntries.map((entry) => (
                                <div
                                  key={entry.id}
                                  className="rounded-lg border border-border/60 p-2 bg-background/80"
                                >
                                  <div
                                    className={`text-sm font-semibold ${courseColorClass(
                                      entry.course_name,
                                    )}`}
                                  >
                                    {entry.course_name}
                                  </div>
                                  <div className="text-xs text-muted-foreground mt-1 space-y-0.5">
                                    {entry.week_text && (
                                      <div>{entry.week_text}</div>
                                    )}
                                    {entry.location && (
                                      <div>{entry.location}</div>
                                    )}
                                    {entry.teacher && (
                                      <div>{entry.teacher}</div>
                                    )}
                                  </div>
                                </div>
                              ))}
                            </div>
                          </td>
                        );
                      })}
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="rounded-2xl border border-border/50 bg-card/40 p-8 text-center text-muted-foreground">
            暂无课表，请先同步
          </div>
        )}
      </div>

      {showSyncModal && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div
            className="w-full max-w-md bg-card border border-border p-6 rounded-2xl shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex justify-between items-center mb-6">
              <h3 className="text-xl font-bold flex items-center gap-2">
                <Calendar className="w-5 h-5 text-primary" />
                同步课程表
              </h3>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setShowSyncModal(false)}
              >
                ×
              </Button>
            </div>
            <div className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">学号</label>
                <input
                  className="w-full px-4 py-2.5 rounded-xl border border-input bg-background focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all"
                  placeholder="请输入学号"
                  value={syncUsername}
                  onChange={(e) => setSyncUsername(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">密码</label>
                <input
                  type="password"
                  className="w-full px-4 py-2.5 rounded-xl border border-input bg-background focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all"
                  placeholder="教务系统密码"
                  value={syncPassword}
                  onChange={(e) => setSyncPassword(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">学期（可选）</label>
                <input
                  className="w-full px-4 py-2.5 rounded-xl border border-input bg-background focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all"
                  placeholder="例如 2025-2026-2"
                  value={syncTerm}
                  onChange={(e) => setSyncTerm(e.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  留空则使用教务系统默认学期
                </p>
              </div>

              {syncMessage && (
                <div className="text-sm text-muted-foreground">
                  {syncMessage}
                </div>
              )}

              <div className="pt-2 flex gap-3">
                <Button
                  variant="outline"
                  className="flex-1 rounded-xl"
                  onClick={() => setShowSyncModal(false)}
                >
                  取消
                </Button>
                <Button
                  className="flex-1 rounded-xl gap-2"
                  onClick={handleSync}
                  disabled={isSyncing}
                >
                  {isSyncing ? "同步中..." : "开始同步"}
                </Button>
              </div>
            </div>
          </div>
        </div>
      )}
    </ToolLayout>
  );
}
