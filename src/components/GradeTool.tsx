import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw,
  UserPlus,
  Users,
  Search,
  GraduationCap,
  TrendingUp,
  BookOpen,
  Clock,
  MoreHorizontal,
  Trash2,
  Edit2,
  X,
  CheckCircle2,
  AlertCircle,
  Filter,
  ArrowUpDown,
  ArrowUp,
  ArrowDown,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ToolLayout } from "@/components/ToolLayout";

// --- Types ---
interface GradeUser {
  username: string;
  display_name?: string | null;
  class_name?: string | null;
  created_at: string;
  last_updated?: string | null;
}

interface GradeRecord {
  id: number;
  username: string;
  term: string;
  course_code: string;
  course_name: string;
  group_name: string;
  score?: string | null;
  score_flag?: string | null;
  credit?: number | null;
  total_hours?: number | null;
  gpa?: number | null;
  makeup_term?: string | null;
  exam_mode?: string | null;
  exam_type?: string | null;
  course_attr?: string | null;
  course_nature?: string | null;
  general_type?: string | null;
  is_minor: boolean;
  updated_at: string;
}

interface PlanCourse {
  id: number;
  term: string;
  course_code: string;
  course_name: string;
  credit?: number | null;
  total_hours?: number | null;
  exam_mode?: string | null;
  course_nature?: string | null;
  course_attr?: string | null;
  is_minor: boolean;
}

interface GradeSyncSummary {
  inserted: number;
  updated: number;
  total: number;
}

// --- Components ---

const StatCard = ({ title, value, icon: Icon, subtext, colorClass }: any) => (
  <div className="relative overflow-hidden rounded-2xl border border-border/50 bg-card/50 p-6 backdrop-blur-sm transition-all hover:shadow-lg hover:border-primary/20 group">
    <div
      className={`absolute right-4 top-4 rounded-full p-2.5 opacity-20 group-hover:opacity-100 transition-opacity ${colorClass.replace("text-", "bg-")}`}
    >
      <Icon className={`w-5 h-5 ${colorClass}`} />
    </div>
    <p className="text-sm font-medium text-muted-foreground">{title}</p>
    <div className="mt-2 flex items-baseline gap-2">
      <span className="text-3xl font-bold tracking-tight">{value}</span>
      {subtext && (
        <span className="text-xs text-muted-foreground">{subtext}</span>
      )}
    </div>
    <div
      className={`absolute bottom-0 left-0 h-1 w-full transform scale-x-0 transition-transform duration-500 group-hover:scale-x-100 ${colorClass.replace("text-", "bg-")}`}
    />
  </div>
);

const Badge = ({
  children,
  variant = "default",
}: {
  children: React.ReactNode;
  variant?: "default" | "success" | "warning" | "danger" | "outline";
}) => {
  const variants = {
    default: "bg-primary/10 text-primary border-primary/20",
    success:
      "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/20",
    warning:
      "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
    danger: "bg-red-500/10 text-red-600 dark:text-red-400 border-red-500/20",
    outline: "bg-transparent border-border text-muted-foreground",
  };
  return (
    <span
      className={`inline-flex items-center rounded-md border px-2 py-1 text-xs font-medium ${variants[variant]}`}
    >
      {children}
    </span>
  );
};

// --- Main Component ---

export default function GradeTool() {
  const navigate = useNavigate();
  // State
  const [users, setUsers] = useState<GradeUser[]>([]);
  const [grades, setGrades] = useState<GradeRecord[]>([]);
  const [pendingCourses, setPendingCourses] = useState<PlanCourse[]>([]);

  const [selectedUser, setSelectedUser] = useState("");
  const [isSyncing, setIsSyncing] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);

  // Filter State
  const [filterText, setFilterText] = useState("");
  const [selectedTerms, setSelectedTerms] = useState<string[]>([]);
  const [gradeCategory, setGradeCategory] = useState<"all" | "major" | "minor">(
    "major",
  );
  const [activeTab, setActiveTab] = useState<"grades" | "pending">("grades");
  const [showFilters, setShowFilters] = useState(false);

  // New Filter State
  const [minCredit, setMinCredit] = useState("");
  const [maxCredit, setMaxCredit] = useState("");
  const [sortConfig, setSortConfig] = useState<{
    key: string;
    direction: "asc" | "desc";
  } | null>(null);

  // Import Form State
  const [importUsername, setImportUsername] = useState("");
  const [importPassword, setImportPassword] = useState("");
  const [importMessage, setImportMessage] = useState("");

  // Helper
  const scoreToNumber = (score?: string | null) => {
    if (!score) return -999;
    const n = parseFloat(score);
    if (!isNaN(n)) return n;
    const map: any = {
      优: 95,
      优秀: 95,
      良: 85,
      良好: 85,
      中: 75,
      中等: 75,
      及格: 65,
      合格: 65,
      通过: 65,
      不及格: 0,
      不合格: 0,
    };
    return map[score.trim()] ?? -999;
  };

  // Loading
  const loadUsers = async () => {
    try {
      const result = await invoke<GradeUser[]>("get_grade_users");
      setUsers(result);
      if (result.length > 0 && !selectedUser) {
        setSelectedUser(result[0].username);
      }
    } catch (error) {
      console.error("Failed to load users:", error);
    }
  };

  const loadGrades = async (user?: string) => {
    if (!user) return;
    try {
      const result = await invoke<GradeRecord[]>("get_grades", {
        username: user,
      });
      setGrades(result);
    } catch (error) {
      console.error("Failed to load grades:", error);
    }
  };

  const loadPendingCourses = async (user: string, category: string) => {
    try {
      const result = await invoke<PlanCourse[]>("get_pending_courses", {
        username: user,
        category,
      });
      setPendingCourses(result);
    } catch (error) {
      console.error("Failed to load pending courses:", error);
    }
  };

  // Effects
  useEffect(() => {
    loadUsers();
  }, []);

  useEffect(() => {
    if (selectedUser) {
      loadGrades(selectedUser);
      loadPendingCourses(selectedUser, gradeCategory);
    } else {
      setGrades([]);
      setPendingCourses([]);
    }
  }, [selectedUser]);

  useEffect(() => {
    if (selectedUser) {
      loadPendingCourses(selectedUser, gradeCategory);
    }
  }, [gradeCategory, selectedUser]);

  // Logic - Sync
  const handleSync = async () => {
    if (!importUsername.trim() || !importPassword.trim()) {
      setImportMessage("请输入账号和密码");
      return;
    }
    setIsSyncing(true);
    setImportMessage("");
    try {
      const result = await invoke<GradeSyncSummary>("sync_grades", {
        request: { username: importUsername.trim(), password: importPassword },
      });
      setImportMessage(
        `同步成功：新增 ${result.inserted}，更新 ${result.updated}`,
      );
      setImportPassword("");
      await loadUsers();
      if (importUsername.trim() === selectedUser) {
        await loadGrades(selectedUser);
        await loadPendingCourses(selectedUser, gradeCategory);
      } else {
        setSelectedUser(importUsername.trim());
      }
      setTimeout(() => setShowImportModal(false), 1500);
    } catch (error) {
      setImportMessage(`同步失败: ${error}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const handleUpdateUser = async (user: GradeUser) => {
    setIsSyncing(true);
    try {
      await invoke("sync_grades_saved", { username: user.username });
      await loadUsers();
      await loadGrades(selectedUser);
      await loadPendingCourses(selectedUser, gradeCategory);
    } catch (error) {
      alert(`更新失败: ${error}`);
    } finally {
      setIsSyncing(false);
    }
  };

  // Logic - Computations
  const termOptions = useMemo(() => {
    const terms = Array.from(new Set(grades.map((item) => item.term)));
    return terms.sort((a, b) => b.localeCompare(a));
  }, [grades]);

  const filteredGrades = useMemo(() => {
    let base = grades.filter(
      (record) => (record.course_attr ?? "").trim() !== "公选",
    );
    if (gradeCategory === "minor")
      base = base.filter((record) => record.is_minor);
    else if (gradeCategory === "major")
      base = base.filter((record) => !record.is_minor);

    if (selectedTerms.length > 0)
      base = base.filter((record) => selectedTerms.includes(record.term));

    // Credit Filter
    if (minCredit !== "") {
      base = base.filter((r) => (r.credit ?? 0) >= parseFloat(minCredit));
    }
    if (maxCredit !== "") {
      base = base.filter((r) => (r.credit ?? 0) <= parseFloat(maxCredit));
    }

    if (filterText.trim()) {
      const keyword = filterText.trim().toLowerCase();
      base = base.filter(
        (record) =>
          record.course_name.toLowerCase().includes(keyword) ||
          record.course_code.toLowerCase().includes(keyword),
      );
    }

    // Sorting
    if (sortConfig && sortConfig.key === "score") {
      base.sort((a, b) => {
        const scoreA = scoreToNumber(a.score);
        const scoreB = scoreToNumber(b.score);

        if (scoreA !== scoreB) {
          if (scoreA < scoreB) return sortConfig.direction === "asc" ? -1 : 1;
          if (scoreA > scoreB) return sortConfig.direction === "asc" ? 1 : -1;
        }

        // Secondary sort: Credit (Always Descending)
        const creditA = a.credit ?? 0;
        const creditB = b.credit ?? 0;
        return creditB - creditA;
      });
    }

    return base;
  }, [
    grades,
    filterText,
    selectedTerms,
    gradeCategory,
    minCredit,
    maxCredit,
    sortConfig,
  ]);

  const stats = useMemo(() => {
    let totalCredits = 0;
    let weightedScore = 0;
    let totalCourses = filteredGrades.length;
    let passedCourses = 0;

    for (const record of filteredGrades) {
      const credit = record.credit ?? 0;
      const scoreNum = scoreToNumber(record.score);

      if (scoreNum !== null && scoreNum >= 60) passedCourses++;

      if (credit > 0 && scoreNum !== null && scoreNum !== -999) {
        totalCredits += credit;
        weightedScore += scoreNum * credit;
      }
    }

    const avg = totalCredits > 0 ? weightedScore / totalCredits : 0;
    return { totalCredits, avg, totalCourses, passedCourses };
  }, [filteredGrades]);

  const handleSort = (key: string) => {
    let direction: "asc" | "desc" = "desc";
    if (
      sortConfig &&
      sortConfig.key === key &&
      sortConfig.direction === "desc"
    ) {
      direction = "asc";
    }
    setSortConfig({ key, direction });
  };

  // Logic - CRUD (Simplified for brevity, reusing prompt logic but wrapping nicer if I had time, sticking to prompts for now)
  const handleEditGrade = async (record: GradeRecord) => {
    const score = window.prompt("修改成绩", record.score || "");
    if (score === null) return;
    // ... minimal impl for demo ...
    try {
      await invoke("update_grade_record", {
        id: record.id,
        score,
        credit: record.credit,
        gpa: record.gpa,
        exam_type: record.exam_type,
        course_attr: record.course_attr,
        course_nature: record.course_nature,
        makeup_term: record.makeup_term,
      });
      loadGrades(selectedUser);
    } catch (e) {
      alert(e);
    }
  };

  const handleDeleteGrade = async (id: number) => {
    if (!window.confirm("确定删除?")) return;
    await invoke("delete_grade_record", { id });
    loadGrades(selectedUser);
  };

  // --- Render ---

  return (
    <ToolLayout
      title="成绩管理"
      description="多维度的成绩分析与管理工具，支持本地数据存储与隐私保护。"
      actions={
        <div className="flex items-center gap-2">
          {/* Account Selector */}
          <div className="relative group">
            <select
              className="appearance-none bg-background border border-input hover:bg-accent hover:text-accent-foreground rounded-full px-4 py-2 pr-8 text-sm font-medium focus:outline-none focus:ring-2 focus:ring-primary transition-all cursor-pointer"
              value={selectedUser}
              onChange={(e) => setSelectedUser(e.target.value)}
            >
              {users.length === 0 && <option>无账号</option>}
              {users.map((u) => (
                <option key={u.username} value={u.username}>
                  {u.username} - {u.display_name || "未命名"}
                </option>
              ))}
            </select>
            <Users className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground pointer-events-none" />
          </div>

          <Button
            onClick={() => setShowImportModal(true)}
            className="gap-2 rounded-full"
          >
            <UserPlus className="w-4 h-4" />
            <span className="hidden sm:inline">导入账号</span>
          </Button>

          {/* Quick Sync Button */}
          {selectedUser && (
            <Button
              variant="outline"
              className="gap-2 rounded-full border-primary/20 hover:bg-primary/10 hover:text-primary"
              onClick={() => {
                const user = users.find((u) => u.username === selectedUser);
                if (user) handleUpdateUser(user);
              }}
              disabled={isSyncing}
            >
              <RefreshCw
                className={`w-4 h-4 ${isSyncing ? "animate-spin" : ""}`}
              />
              <span className="hidden sm:inline">
                {isSyncing ? "更新中..." : "更新数据"}
              </span>
            </Button>
          )}
        </div>
      }
    >
      <div className="space-y-8">
        {/* Stats Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <StatCard
            title="平均学分绩 (Weighted)"
            value={stats.avg.toFixed(2)}
            icon={TrendingUp}
            colorClass="text-emerald-500"
          />
          <StatCard
            title="已修总学分"
            value={stats.totalCredits.toFixed(1)}
            icon={BookOpen}
            colorClass="text-blue-500"
          />
          <StatCard
            title="课程总数"
            value={stats.totalCourses}
            subtext={`通过率 ${stats.totalCourses ? Math.round((stats.passedCourses / stats.totalCourses) * 100) : 0}%`}
            icon={GraduationCap}
            colorClass="text-violet-500"
          />
          <StatCard
            title="最近更新"
            value={
              users.find((u) => u.username === selectedUser)?.last_updated
                ? new Date(
                    users.find((u) => u.username === selectedUser)
                      ?.last_updated!,
                  ).toLocaleDateString()
                : "Never"
            }
            icon={Clock}
            colorClass="text-amber-500"
          />
        </div>

        {/* Toolbar & Filters */}
        <div className="flex flex-col md:flex-row gap-4 justify-between items-center bg-card/30 p-4 rounded-2xl border border-border/50">
          {/* Left: Tabs */}
          <div className="flex bg-muted/50 p-1 rounded-xl">
            <button
              onClick={() => setActiveTab("grades")}
              className={`px-4 py-2 rounded-lg text-sm font-medium transition-all ${activeTab === "grades" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"}`}
            >
              成绩列表
            </button>
            <button
              onClick={() => setActiveTab("pending")}
              className={`px-4 py-2 rounded-lg text-sm font-medium transition-all ${activeTab === "pending" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"}`}
            >
              待修课程
            </button>
          </div>

          {/* Right: Filters */}
          <div className="flex flex-wrap items-center gap-3 w-full md:w-auto">
            {/* Category Toggle */}
            <div className="flex items-center border border-border rounded-lg overflow-hidden h-9">
              <button
                onClick={() => setGradeCategory("major")}
                className={`px-3 h-full text-xs font-medium transition-colors ${gradeCategory === "major" ? "bg-primary/10 text-primary" : "hover:bg-muted"}`}
              >
                主修
              </button>
              <div className="w-px h-full bg-border" />
              <button
                onClick={() => setGradeCategory("minor")}
                className={`px-3 h-full text-xs font-medium transition-colors ${gradeCategory === "minor" ? "bg-primary/10 text-primary" : "hover:bg-muted"}`}
              >
                辅修
              </button>
              <div className="w-px h-full bg-border" />
              <button
                onClick={() => setGradeCategory("all")}
                className={`px-3 h-full text-xs font-medium transition-colors ${gradeCategory === "all" ? "bg-primary/10 text-primary" : "hover:bg-muted"}`}
              >
                全部
              </button>
            </div>

            {/* Term Select */}
            <div className="relative">
              <Filter className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
              <select
                multiple={false} // Simplify to single select for UI cleaness or keep custom multiple. Let's do simple single select for now or a better dropdown
                className="h-9 pl-8 pr-8 border border-border bg-transparent rounded-lg text-xs focus:ring-1 focus:ring-primary appearance-none outline-none"
                onChange={(e) => {
                  const val = e.target.value;
                  setSelectedTerms(val ? [val] : []);
                }}
                value={selectedTerms[0] || ""}
              >
                <option value="">所有学期</option>
                {termOptions.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </div>

            {/* Search */}
            <div className="relative flex-1 md:w-64">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <input
                type="text"
                placeholder="搜索课程..."
                value={filterText}
                onChange={(e) => setFilterText(e.target.value)}
                className="w-full h-9 pl-9 pr-4 rounded-lg border border-border bg-transparent text-sm focus:outline-none focus:ring-1 focus:ring-primary transition-all placeholder:text-muted-foreground/50"
              />
            </div>

            {/* Advanced Filter Toggle */}
            <Button
              variant={showFilters ? "default" : "outline"}
              size="sm"
              className="h-9 px-3 gap-2"
              onClick={() => setShowFilters(!showFilters)}
            >
              <Filter className="w-4 h-4" />
              <span className="hidden sm:inline">筛选</span>
            </Button>
          </div>
        </div>

        {/* Extended Filters Panel */}
        {showFilters && (
          <div className="bg-card/30 p-4 rounded-2xl border border-border/50 animate-in slide-in-from-top-2 duration-200">
            <div className="flex items-center gap-4 text-sm">
              <span className="font-medium text-muted-foreground">
                学分范围:
              </span>
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  placeholder="Min"
                  className="w-20 h-8 px-2 rounded-md border border-border bg-background focus:outline-none focus:ring-1 focus:ring-primary"
                  value={minCredit}
                  onChange={(e) => setMinCredit(e.target.value)}
                />
                <span className="text-muted-foreground">-</span>
                <input
                  type="number"
                  placeholder="Max"
                  className="w-20 h-8 px-2 rounded-md border border-border bg-background focus:outline-none focus:ring-1 focus:ring-primary"
                  value={maxCredit}
                  onChange={(e) => setMaxCredit(e.target.value)}
                />
              </div>
              {(minCredit || maxCredit) && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-8 px-2 text-muted-foreground hover:text-destructive"
                  onClick={() => {
                    setMinCredit("");
                    setMaxCredit("");
                  }}
                >
                  清除
                </Button>
              )}
            </div>
          </div>
        )}

        {/* Content Table */}
        <div className="rounded-2xl border border-border/50 overflow-hidden bg-card/40 backdrop-blur-md shadow-sm">
          {activeTab === "grades" ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm text-left">
                <thead className="bg-muted/50 text-muted-foreground font-medium border-b border-border/50">
                  <tr>
                    <th className="px-6 py-4">课程名称</th>
                    <th
                      className="px-6 py-4 cursor-pointer hover:text-foreground hover:bg-muted/50 transition-colors select-none group"
                      onClick={() => handleSort("score")}
                    >
                      <div className="flex items-center gap-1">
                        成绩
                        {sortConfig?.key === "score" ? (
                          sortConfig.direction === "asc" ? (
                            <ArrowUp className="w-3.5 h-3.5 text-primary" />
                          ) : (
                            <ArrowDown className="w-3.5 h-3.5 text-primary" />
                          )
                        ) : (
                          <ArrowUpDown className="w-3.5 h-3.5 opacity-0 group-hover:opacity-50 transition-opacity" />
                        )}
                      </div>
                    </th>
                    <th className="px-6 py-4">学分</th>
                    <th className="px-6 py-4">绩点</th>
                    <th className="px-6 py-4">属性</th>
                    <th className="px-6 py-4">学期</th>
                    <th className="px-6 py-4 text-right">操作</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border/30">
                  {filteredGrades.length === 0 ? (
                    <tr>
                      <td
                        colSpan={7}
                        className="px-6 py-12 text-center text-muted-foreground"
                      >
                        <div className="flex flex-col items-center gap-2">
                          <Search className="w-8 h-8 opacity-20" />
                          <p>暂无数据</p>
                        </div>
                      </td>
                    </tr>
                  ) : (
                    filteredGrades.map((record) => {
                      const scoreNum = parseFloat(record.score || "0");
                      const isPass = !isNaN(scoreNum) ? scoreNum >= 60 : true; // Simple logic

                      return (
                        <tr
                          key={record.id}
                          className="hover:bg-muted/30 transition-colors group"
                        >
                          <td className="px-6 py-4">
                            <div className="font-medium text-foreground">
                              {record.course_name}
                            </div>
                            <div className="text-xs text-muted-foreground font-mono mt-0.5">
                              {record.course_code}
                            </div>
                          </td>
                          <td className="px-6 py-4">
                            {record.score ? (
                              <Badge
                                variant={
                                  isPass
                                    ? scoreNum >= 90
                                      ? "success"
                                      : "default"
                                    : "danger"
                                }
                              >
                                {record.score}
                              </Badge>
                            ) : (
                              <span className="text-muted-foreground">-</span>
                            )}
                          </td>
                          <td className="px-6 py-4">{record.credit}</td>
                          <td className="px-6 py-4 font-mono text-muted-foreground">
                            {record.gpa}
                          </td>
                          <td className="px-6 py-4">
                            <div className="flex gap-1 flex-wrap">
                              <span className="text-xs border border-border px-1.5 py-0.5 rounded text-muted-foreground">
                                {record.course_attr}
                              </span>
                              {record.is_minor && (
                                <span className="text-xs bg-amber-500/10 text-amber-500 px-1.5 py-0.5 rounded">
                                  辅修
                                </span>
                              )}
                            </div>
                          </td>
                          <td className="px-6 py-4 text-muted-foreground">
                            {record.term}
                          </td>
                          <td className="px-6 py-4 text-right">
                            <div className="flex items-center justify-end gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                              <Button
                                size="icon"
                                variant="ghost"
                                className="h-8 w-8 text-muted-foreground hover:text-primary"
                                onClick={() => handleEditGrade(record)}
                              >
                                <Edit2 className="w-4 h-4" />
                              </Button>
                              <Button
                                size="icon"
                                variant="ghost"
                                className="h-8 w-8 text-muted-foreground hover:text-destructive"
                                onClick={() => handleDeleteGrade(record.id)}
                              >
                                <Trash2 className="w-4 h-4" />
                              </Button>
                            </div>
                          </td>
                        </tr>
                      );
                    })
                  )}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm text-left">
                <thead className="bg-muted/50 text-muted-foreground font-medium border-b border-border/50">
                  <tr>
                    <th className="px-6 py-4">学期</th>
                    <th className="px-6 py-4">课程名称</th>
                    <th className="px-6 py-4">学分</th>
                    <th className="px-6 py-4">总学时</th>
                    <th className="px-6 py-4">属性</th>
                    <th className="px-6 py-4">考核方式</th>
                    <th className="px-6 py-4 text-right">操作</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border/30">
                  {pendingCourses.length === 0 ? (
                    <tr>
                      <td
                        colSpan={7}
                        className="px-6 py-12 text-center text-muted-foreground"
                      >
                        <div className="flex flex-col items-center gap-2">
                          <Search className="w-8 h-8 opacity-20" />
                          <p>暂无待修课程</p>
                        </div>
                      </td>
                    </tr>
                  ) : (
                    pendingCourses.map((course) => (
                      <tr
                        key={`${course.term}-${course.course_code}`}
                        className="hover:bg-muted/30 transition-colors group"
                      >
                        <td className="px-6 py-4 text-muted-foreground">
                          {course.term}
                        </td>
                        <td className="px-6 py-4">
                          <div className="font-medium text-foreground">
                            {course.course_name}
                          </div>
                          <div className="text-xs text-muted-foreground font-mono mt-0.5">
                            {course.course_code}
                          </div>
                        </td>
                        <td className="px-6 py-4">{course.credit}</td>
                        <td className="px-6 py-4">{course.total_hours}</td>
                        <td className="px-6 py-4">
                          <div className="flex gap-1 flex-wrap">
                            <span className="text-xs border border-border px-1.5 py-0.5 rounded text-muted-foreground">
                              {course.course_attr}
                            </span>
                            <span className="text-xs border border-border px-1.5 py-0.5 rounded text-muted-foreground">
                              {course.course_nature}
                            </span>
                            {course.is_minor && (
                              <span className="text-xs bg-amber-500/10 text-amber-500 px-1.5 py-0.5 rounded">
                                辅修
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="px-6 py-4 text-muted-foreground">
                          {course.exam_mode}
                        </td>
                        <td className="px-6 py-4 text-right">
                          <div className="flex items-center justify-end gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                            {/* Edit/Delete actions could be added here similar to grades if needed */}
                            <span className="text-xs text-muted-foreground italic">
                              暂无操作
                            </span>
                          </div>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* Import Modal (Simple Overlay) */}
      {showImportModal && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm animate-in fade-in duration-200">
          <div
            className="w-full max-w-md bg-card border border-border p-6 rounded-2xl shadow-2xl animate-in zoom-in-95 duration-200"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex justify-between items-center mb-6">
              <h3 className="text-xl font-bold flex items-center gap-2">
                <RefreshCw className="w-5 h-5 text-primary" />
                同步教务成绩
              </h3>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setShowImportModal(false)}
              >
                <X className="w-5 h-5" />
              </Button>
            </div>

            <div className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">学号</label>
                <input
                  className="w-full px-4 py-2.5 rounded-xl border border-input bg-background focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all"
                  placeholder="请输入学号"
                  value={importUsername}
                  onChange={(e) => setImportUsername(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">密码</label>
                <input
                  type="password"
                  className="w-full px-4 py-2.5 rounded-xl border border-input bg-background focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all"
                  placeholder="教务系统密码"
                  value={importPassword}
                  onChange={(e) => setImportPassword(e.target.value)}
                />
              </div>

              {importMessage && (
                <div
                  className={`p-3 rounded-lg text-sm flex items-center gap-2 ${importMessage.includes("失败") ? "bg-destructive/10 text-destructive" : "bg-emerald-500/10 text-emerald-600"}`}
                >
                  {importMessage.includes("失败") ? (
                    <AlertCircle className="w-4 h-4" />
                  ) : (
                    <CheckCircle2 className="w-4 h-4" />
                  )}
                  {importMessage}
                </div>
              )}

              <div className="pt-2 flex gap-3">
                <Button
                  variant="outline"
                  className="flex-1 rounded-xl"
                  onClick={() => setShowImportModal(false)}
                >
                  取消
                </Button>
                <Button
                  className="flex-1 rounded-xl gap-2"
                  onClick={handleSync}
                  disabled={isSyncing}
                >
                  {isSyncing ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : null}
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
