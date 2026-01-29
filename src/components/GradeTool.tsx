import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft,
  RefreshCw,
  UserPlus,
  Users,
  Search,
  GraduationCap,
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

export default function GradeTool() {
  const navigate = useNavigate();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [isSyncing, setIsSyncing] = useState(false);
  const [message, setMessage] = useState("");
  const [users, setUsers] = useState<GradeUser[]>([]);
  const [grades, setGrades] = useState<GradeRecord[]>([]);
  const [selectedUser, setSelectedUser] = useState("");
  const [filterText, setFilterText] = useState("");
  const [selectedTerms, setSelectedTerms] = useState<string[]>([]);
  const [gradeCategory, setGradeCategory] = useState<"all" | "major" | "minor">(
    "major"
  );
  const [pendingCourses, setPendingCourses] = useState<PlanCourse[]>([]);
  const [isPendingLoading, setIsPendingLoading] = useState(false);

  const loadUsers = async () => {
    try {
      const result = await invoke<GradeUser[]>("get_grade_users");
      setUsers(result);
    } catch (error) {
      console.error("Failed to load users:", error);
    }
  };

  const loadGrades = async (user?: string) => {
    try {
      const result = await invoke<GradeRecord[]>("get_grades", {
        username: user && user.trim() ? user : null,
      });
      setGrades(result);
    } catch (error) {
      console.error("Failed to load grades:", error);
    }
  };

  const loadPendingCourses = async (user: string, category: string) => {
    setIsPendingLoading(true);
    try {
      const result = await invoke<PlanCourse[]>("get_pending_courses", {
        username: user,
        category,
      });
      setPendingCourses(result);
    } catch (error) {
      console.error("Failed to load pending courses:", error);
    } finally {
      setIsPendingLoading(false);
    }
  };

  useEffect(() => {
    loadUsers();
    setGrades([]);
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

  const syncWithCredentials = async (user: string, pass: string) => {
    if (!user.trim() || !pass.trim()) {
      setMessage("请输入账号和密码");
      return;
    }
    setIsSyncing(true);
    setMessage("");
    try {
      const result = await invoke<GradeSyncSummary>("sync_grades", {
        request: {
          username: user.trim(),
          password: pass,
        },
      });
      setMessage(
        `同步完成：新增 ${result.inserted} 条，更新 ${result.updated} 条，总计 ${result.total} 条`
      );
      setPassword("");
      await loadUsers();
      if (selectedUser) {
        await loadGrades(selectedUser);
        await loadPendingCourses(selectedUser, gradeCategory);
      }
    } catch (error) {
      setMessage(`同步失败: ${error}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const handleAddAndSync = async () => {
    await syncWithCredentials(username, password);
  };

  const handleUpdateUser = async (user: GradeUser) => {
    setIsSyncing(true);
    setMessage("");
    try {
      const result = await invoke<GradeSyncSummary>("sync_grades_saved", {
        username: user.username,
      });
      setMessage(
        `同步完成：新增 ${result.inserted} 条，更新 ${result.updated} 条，总计 ${result.total} 条`
      );
      await loadUsers();
      await loadGrades(selectedUser);
      if (selectedUser) {
        await loadPendingCourses(selectedUser, gradeCategory);
      }
    } catch (error) {
      setMessage(`同步失败: ${error}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const handleEditUser = async (user: GradeUser) => {
    const name = window.prompt("姓名", user.display_name ?? "");
    if (name === null) return;
    const className = window.prompt("班级", user.class_name ?? "");
    if (className === null) return;
    const passwordDate = window.prompt("密码/日期", "");
    if (passwordDate === null) return;
    try {
      await invoke("update_password_result", {
        username: user.username,
        name: name.trim() ? name.trim() : null,
        class_name: className.trim() ? className.trim() : null,
        password_date: passwordDate.trim() ? passwordDate.trim() : null,
      });
      await loadUsers();
      setMessage("已更新用户信息");
    } catch (error) {
      setMessage(`更新失败: ${error}`);
    }
  };

  const handleDeleteUser = async (user: GradeUser) => {
    const ok = window.confirm(
      `确定隐藏 ${user.username} 并删除该账号成绩记录吗？`
    );
    if (!ok) return;
    setIsSyncing(true);
    setMessage("");
    try {
      await invoke("hide_grade_user", { username: user.username });
      if (selectedUser === user.username) {
        setSelectedUser("");
        setGrades([]);
        setPendingCourses([]);
      }
      await loadUsers();
      setMessage("已删除该账号成绩记录");
    } catch (error) {
      setMessage(`删除失败: ${error}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const handleDeleteUserRecord = async (user: GradeUser) => {
    const ok = window.confirm(`确定删除账号 ${user.username} 吗？`);
    if (!ok) return;
    setIsSyncing(true);
    setMessage("");
    try {
      await invoke("delete_password_result", { username: user.username });
      if (selectedUser === user.username) {
        setSelectedUser("");
        setGrades([]);
        setPendingCourses([]);
      }
      await loadUsers();
      setMessage("已删除账号记录");
    } catch (error) {
      setMessage(`删除失败: ${error}`);
    } finally {
      setIsSyncing(false);
    }
  };

  const handleEditGrade = async (record: GradeRecord) => {
    const score = window.prompt("成绩", record.score ?? "");
    if (score === null) return;
    const creditText = window.prompt(
      "学分",
      record.credit !== null && record.credit !== undefined
        ? String(record.credit)
        : ""
    );
    if (creditText === null) return;
    const gpaText = window.prompt(
      "绩点",
      record.gpa !== null && record.gpa !== undefined ? String(record.gpa) : ""
    );
    if (gpaText === null) return;
    const examType = window.prompt("考试性质", record.exam_type ?? "");
    if (examType === null) return;
    const courseAttr = window.prompt("课程属性", record.course_attr ?? "");
    if (courseAttr === null) return;
    const courseNature = window.prompt("课程性质", record.course_nature ?? "");
    if (courseNature === null) return;
    const makeupTerm = window.prompt("补重学期", record.makeup_term ?? "");
    if (makeupTerm === null) return;

    const credit = creditText.trim() ? Number.parseFloat(creditText) : null;
    const gpa = gpaText.trim() ? Number.parseFloat(gpaText) : null;
    try {
      await invoke("update_grade_record", {
        id: record.id,
        score: score.trim() ? score.trim() : null,
        credit: Number.isNaN(credit) ? null : credit,
        gpa: Number.isNaN(gpa) ? null : gpa,
        exam_type: examType.trim() ? examType.trim() : null,
        course_attr: courseAttr.trim() ? courseAttr.trim() : null,
        course_nature: courseNature.trim() ? courseNature.trim() : null,
        makeup_term: makeupTerm.trim() ? makeupTerm.trim() : null,
      });
      if (selectedUser) {
        await loadGrades(selectedUser);
      }
      setMessage("已更新成绩记录");
    } catch (error) {
      setMessage(`更新失败: ${error}`);
    }
  };

  const handleDeleteGrade = async (record: GradeRecord) => {
    const ok = window.confirm(`确定删除 ${record.course_name} 这条成绩吗？`);
    if (!ok) return;
    try {
      await invoke("delete_grade_record", { id: record.id });
      if (selectedUser) {
        await loadGrades(selectedUser);
      }
      setMessage("已删除成绩记录");
    } catch (error) {
      setMessage(`删除失败: ${error}`);
    }
  };

  const handleEditPlanCourse = async (item: PlanCourse) => {
    const courseName = window.prompt("课程名称", item.course_name ?? "");
    if (courseName === null) return;
    const creditText = window.prompt(
      "学分",
      item.credit !== null && item.credit !== undefined
        ? String(item.credit)
        : ""
    );
    if (creditText === null) return;
    const totalText = window.prompt(
      "总学时",
      item.total_hours !== null && item.total_hours !== undefined
        ? String(item.total_hours)
        : ""
    );
    if (totalText === null) return;
    const examMode = window.prompt("考核方式", item.exam_mode ?? "");
    if (examMode === null) return;
    const courseNature = window.prompt("课程性质", item.course_nature ?? "");
    if (courseNature === null) return;
    const courseAttr = window.prompt("课程属性", item.course_attr ?? "");
    if (courseAttr === null) return;
    const credit = creditText.trim() ? Number.parseFloat(creditText) : null;
    const totalHours = totalText.trim() ? Number.parseFloat(totalText) : null;
    try {
      await invoke("update_plan_course", {
        id: item.id,
        course_name: courseName.trim() ? courseName.trim() : null,
        credit: Number.isNaN(credit) ? null : credit,
        total_hours: Number.isNaN(totalHours) ? null : totalHours,
        exam_mode: examMode.trim() ? examMode.trim() : null,
        course_nature: courseNature.trim() ? courseNature.trim() : null,
        course_attr: courseAttr.trim() ? courseAttr.trim() : null,
      });
      if (selectedUser) {
        await loadPendingCourses(selectedUser, gradeCategory);
      }
      setMessage("已更新待修课程");
    } catch (error) {
      setMessage(`更新失败: ${error}`);
    }
  };

  const handleDeletePlanCourse = async (item: PlanCourse) => {
    const ok = window.confirm(`确定删除 ${item.course_name} 这条待修课程吗？`);
    if (!ok) return;
    try {
      await invoke("delete_plan_course", { id: item.id });
      if (selectedUser) {
        await loadPendingCourses(selectedUser, gradeCategory);
      }
      setMessage("已删除待修课程");
    } catch (error) {
      setMessage(`删除失败: ${error}`);
    }
  };

  const termOptions = useMemo(() => {
    const terms = Array.from(new Set(grades.map((item) => item.term)));
    return terms.sort((a, b) => b.localeCompare(a));
  }, [grades]);

  const filteredGrades = useMemo(() => {
    let base = grades.filter(
      (record) => (record.course_attr ?? "").trim() !== "公选"
    );
    if (gradeCategory === "minor") {
      base = base.filter((record) => record.is_minor);
    } else if (gradeCategory === "major") {
      base = base.filter((record) => !record.is_minor);
    }
    if (selectedTerms.length > 0) {
      base = base.filter((record) => selectedTerms.includes(record.term));
    }
    if (!filterText.trim()) return base;
    const keyword = filterText.trim().toLowerCase();
    return base.filter((record) => {
      return (
        record.term.toLowerCase().includes(keyword) ||
        record.course_code.toLowerCase().includes(keyword) ||
        record.course_name.toLowerCase().includes(keyword) ||
        (record.score ?? "").toLowerCase().includes(keyword) ||
        record.username.toLowerCase().includes(keyword)
      );
    });
  }, [grades, filterText, selectedTerms, gradeCategory]);

  const scoreToNumber = (score?: string | null) => {
    if (!score) return null;
    const normalized = score.trim();
    if (!normalized) return null;
    const numeric = Number.parseFloat(normalized);
    if (!Number.isNaN(numeric)) return numeric;
    const map: Record<string, number> = {
      优: 95,
      优秀: 95,
      良: 85,
      良好: 85,
      中: 75,
      中等: 75,
      及格: 65,
      合格: 65,
      通过: 65,
      不及格: 50,
      不合格: 50,
      未通过: 50,
    };
    return map[normalized] ?? null;
  };

  const summary = useMemo(() => {
    let totalCredits = 0;
    let weightedScore = 0;
    for (const record of filteredGrades) {
      const credit = record.credit ?? 0;
      if (!credit || credit <= 0) continue;
      totalCredits += credit;
      const scoreNumber = scoreToNumber(record.score);
      if (scoreNumber !== null) {
        weightedScore += scoreNumber * credit;
      }
    }
    const averageScore =
      totalCredits > 0 ? weightedScore / totalCredits : null;
    return { totalCredits, averageScore };
  }, [filteredGrades]);

  const filteredPending = useMemo(() => {
    if (selectedTerms.length === 0) return pendingCourses;
    return pendingCourses.filter((item) => selectedTerms.includes(item.term));
  }, [pendingCourses, selectedTerms]);

  const pendingGroups = useMemo(() => {
    const groups: Record<"必修" | "选修" | "其他", PlanCourse[]> = {
      必修: [],
      选修: [],
      其他: [],
    };
    for (const item of filteredPending) {
      const attr = (item.course_attr ?? "").trim();
      if (attr === "必修" || attr === "必修课") {
        groups["必修"].push(item);
      } else if (attr === "选修" || attr === "选修课") {
        groups["选修"].push(item);
      } else {
        groups["其他"].push(item);
      }
    }
    return groups;
  }, [filteredPending]);

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
          <span className="p-2 bg-emerald-500/10 text-emerald-500 rounded-lg">
            <GraduationCap className="w-8 h-8" />
          </span>
          成绩查询
        </h1>
        <p className="text-muted-foreground">
          登录教务系统拉取最新成绩，仅保存本地记录
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <Card className="lg:col-span-2 shadow-lg shadow-emerald-500/5">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <UserPlus className="w-5 h-5 text-emerald-500" />
              导入/更新成绩
            </CardTitle>
            <CardDescription>输入账号密码导入成绩</CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div className="space-y-2">
                <label className="text-sm font-medium text-foreground">账号</label>
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  disabled={isSyncing}
                  placeholder="输入学号"
                  className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-emerald-500 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium text-foreground">密码</label>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={isSyncing}
                  placeholder="输入教务系统密码"
                  className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-emerald-500 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                />
              </div>
            </div>

            <Button
              onClick={handleAddAndSync}
              disabled={isSyncing}
              className="w-full gap-2 bg-emerald-600 hover:bg-emerald-700 text-white shadow-lg shadow-emerald-500/20 h-11 text-lg font-semibold"
            >
              <RefreshCw className={`w-5 h-5 ${isSyncing ? "animate-spin" : ""}`} />
              {isSyncing ? "同步中..." : "开始同步"}
            </Button>

            {message && (
              <div
                className={`p-3 rounded-lg border ${
                  message.includes("失败") || message.includes("错误")
                    ? "bg-destructive/10 border-destructive/20 text-destructive"
                    : "bg-emerald-500/10 border-emerald-500/20 text-emerald-600"
                }`}
              >
                <p className="text-sm font-medium">{message}</p>
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="h-fit">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Users className="w-5 h-5 text-emerald-500" />
              已添加账号
            </CardTitle>
            <CardDescription>点击更新拉取最新成绩</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {users.length === 0 ? (
              <div className="py-6 text-center text-muted-foreground">
                暂无账号
              </div>
            ) : (
              <div className="space-y-3">
                {users.map((user) => (
                  <div
                    key={user.username}
                    className="flex items-center justify-between gap-3 rounded-lg border border-border/60 bg-muted/20 px-3 py-2"
                  >
                    <div className="min-w-0">
                    <button
                      type="button"
                      className="text-left"
                      onClick={() => setSelectedUser(user.username)}
                    >
                      <p className="text-sm font-semibold text-foreground">
                        {user.username}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {[user.display_name, user.class_name]
                          .filter(Boolean)
                          .join(" · ") || "未设置姓名"}
                      </p>
                    </button>
                      <p className="text-xs text-muted-foreground">
                        上次更新：
                        {user.last_updated
                          ? new Date(user.last_updated).toLocaleString()
                          : "未同步"}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => setSelectedUser(user.username)}
                      >
                        查看
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleUpdateUser(user)}
                      >
                        更新
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleEditUser(user)}
                      >
                        编辑
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleDeleteUserRecord(user)}
                      >
                        删除
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Search className="w-5 h-5 text-emerald-500" />
            成绩记录
          </CardTitle>
          <CardDescription>
            {selectedUser ? `当前账号：${selectedUser}` : "选择账号查看成绩"}
          </CardDescription>
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
                  placeholder="搜索课程/学期/成绩/学号..."
                  className="w-full pl-9 pr-4 py-2 border border-input rounded-md bg-background text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-emerald-500 transition-all"
                />
              </div>
            </div>
            <div className="md:col-span-4">
              <div className="grid grid-cols-3 gap-2">
                <Button
                  variant={gradeCategory === "major" ? "default" : "outline"}
                  className="w-full"
                  onClick={() => setGradeCategory("major")}
                >
                  主修
                </Button>
                <Button
                  variant={gradeCategory === "minor" ? "default" : "outline"}
                  className="w-full"
                  onClick={() => setGradeCategory("minor")}
                >
                  辅修
                </Button>
                <Button
                  variant={gradeCategory === "all" ? "default" : "outline"}
                  className="w-full"
                  onClick={() => setGradeCategory("all")}
                >
                  全部
                </Button>
              </div>
              <p className="text-xs text-muted-foreground mt-2">
                当前统计基于 {gradeCategory === "all"
                  ? "全部"
                  : gradeCategory === "minor"
                  ? "辅修"
                  : "主修"} 列表
              </p>
            </div>
            <div className="md:col-span-4">
              <select
                multiple
                value={selectedTerms}
                onChange={(e) => {
                  const values = Array.from(e.target.selectedOptions).map(
                    (option) => option.value
                  );
                  setSelectedTerms(values);
                }}
                className="w-full px-3 py-2 border border-input rounded-md bg-background text-foreground focus:outline-none focus:ring-2 focus:ring-emerald-500 transition-all h-28"
              >
                {termOptions.map((term) => (
                  <option key={term} value={term}>
                    {term}
                  </option>
                ))}
              </select>
              {termOptions.length === 0 && (
                <p className="text-xs text-muted-foreground mt-2">
                  暂无学期可选
                </p>
              )}
              {selectedTerms.length > 0 && (
                <Button
                  size="sm"
                  variant="ghost"
                  className="mt-2 px-2"
                  onClick={() => setSelectedTerms([])}
                >
                  清空学期筛选
                </Button>
              )}
            </div>
            <div className="md:col-span-3 flex items-center">
              <Button
                variant="outline"
                className="w-full gap-2"
                onClick={() => selectedUser && loadGrades(selectedUser)}
                disabled={!selectedUser}
              >
                <RefreshCw className="w-4 h-4" />
                刷新列表
              </Button>
            </div>
          </div>

          {!selectedUser ? (
            <div className="text-center py-12 text-muted-foreground bg-muted/10 rounded-xl border border-dashed border-border">
              <Users className="w-8 h-8 mx-auto mb-2 opacity-50" />
              请先在右侧选择一个账号
            </div>
          ) : filteredGrades.length === 0 ? (
            <div className="text-center py-12 text-muted-foreground bg-muted/10 rounded-xl border border-dashed border-border">
              <Search className="w-8 h-8 mx-auto mb-2 opacity-50" />
              没有符合条件的记录
            </div>
          ) : (
            <div className="space-y-4">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="rounded-lg border border-border/60 bg-muted/20 px-4 py-3">
                  <p className="text-xs text-muted-foreground">总学分</p>
                  <p className="text-lg font-semibold text-foreground">
                    {summary.totalCredits.toFixed(2)}
                  </p>
                </div>
                <div className="rounded-lg border border-border/60 bg-muted/20 px-4 py-3">
                  <p className="text-xs text-muted-foreground">平均学分绩</p>
                  <p className="text-lg font-semibold text-foreground">
                    {summary.averageScore !== null
                      ? summary.averageScore.toFixed(2)
                      : "—"}
                  </p>
                </div>
              </div>

              <div className="rounded-xl border border-border overflow-auto">
              <table className="w-full text-sm min-w-[1600px]">
                <thead className="bg-muted/50">
                  <tr className="text-left text-muted-foreground">
                    <th className="py-3 px-4 font-medium">学号</th>
                    <th className="py-3 px-4 font-medium">学期</th>
                    <th className="py-3 px-4 font-medium">课程编号</th>
                    <th className="py-3 px-4 font-medium">课程名称</th>
                    <th className="py-3 px-4 font-medium">成绩</th>
                    <th className="py-3 px-4 font-medium">成绩标识</th>
                    <th className="py-3 px-4 font-medium">学分</th>
                    <th className="py-3 px-4 font-medium">总学时</th>
                    <th className="py-3 px-4 font-medium">绩点</th>
                    <th className="py-3 px-4 font-medium">补重学期</th>
                    <th className="py-3 px-4 font-medium">考试性质</th>
                    <th className="py-3 px-4 font-medium">课程属性</th>
                    <th className="py-3 px-4 font-medium">课程性质</th>
                    <th className="py-3 px-4 font-medium">操作</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border/50">
                  {filteredGrades.map((record) => (
                    <tr key={record.id} className="hover:bg-muted/30 transition-colors">
                      <td className="py-3 px-4 font-mono">{record.username}</td>
                      <td className="py-3 px-4">{record.term}</td>
                      <td className="py-3 px-4 font-mono">{record.course_code}</td>
                      <td className="py-3 px-4">{record.course_name}</td>
                      <td className="py-3 px-4">
                        {record.score ? (
                          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-400">
                            {record.score}
                          </span>
                        ) : (
                          <span className="text-muted-foreground">—</span>
                        )}
                      </td>
                      <td className="py-3 px-4">{record.score_flag ?? "—"}</td>
                      <td className="py-3 px-4">{record.credit ?? "—"}</td>
                      <td className="py-3 px-4">{record.total_hours ?? "—"}</td>
                      <td className="py-3 px-4">{record.gpa ?? "—"}</td>
                      <td className="py-3 px-4">{record.makeup_term ?? "—"}</td>
                      <td className="py-3 px-4">{record.exam_type ?? "—"}</td>
                      <td className="py-3 px-4">{record.course_attr ?? "—"}</td>
                      <td className="py-3 px-4">{record.course_nature ?? "—"}</td>
                      <td className="py-3 px-4">
                        <div className="flex items-center gap-2">
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleEditGrade(record)}
                          >
                            编辑
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleDeleteGrade(record)}
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
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <GraduationCap className="w-5 h-5 text-emerald-500" />
            待修课程
          </CardTitle>
          <CardDescription>执行计划中有但尚未出成绩的课程</CardDescription>
        </CardHeader>
        <CardContent>
          {!selectedUser ? (
            <div className="text-center py-12 text-muted-foreground bg-muted/10 rounded-xl border border-dashed border-border">
              <Users className="w-8 h-8 mx-auto mb-2 opacity-50" />
              请先在右侧选择一个账号
            </div>
          ) : isPendingLoading ? (
            <div className="text-center py-12 text-muted-foreground">加载中...</div>
          ) : filteredPending.length === 0 ? (
            <div className="text-center py-12 text-muted-foreground bg-muted/10 rounded-xl border border-dashed border-border">
              暂无待修课程
            </div>
          ) : (
            <div className="space-y-6">
              {(["必修", "选修", "其他"] as const).map((group) => {
                const items = pendingGroups[group];
                if (items.length === 0) return null;
                return (
                  <div key={group} className="space-y-2">
                    <div className="flex items-center justify-between">
                      <h3 className="text-sm font-semibold text-foreground">
                        {group}
                      </h3>
                      <span className="text-xs text-muted-foreground">
                        {items.length} 门
                      </span>
                    </div>
                    <div className="rounded-xl border border-border overflow-auto">
                      <table className="w-full text-sm min-w-[900px]">
                        <thead className="bg-muted/50">
                          <tr className="text-left text-muted-foreground">
                            {gradeCategory === "all" && (
                              <th className="py-3 px-4 font-medium">类别</th>
                            )}
                            <th className="py-3 px-4 font-medium">学期</th>
                            <th className="py-3 px-4 font-medium">课程编号</th>
                            <th className="py-3 px-4 font-medium">课程名称</th>
                            <th className="py-3 px-4 font-medium">学分</th>
                            <th className="py-3 px-4 font-medium">总学时</th>
                            <th className="py-3 px-4 font-medium">考核方式</th>
                            <th className="py-3 px-4 font-medium">课程性质</th>
                            <th className="py-3 px-4 font-medium">课程属性</th>
                            <th className="py-3 px-4 font-medium">操作</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-border/50">
                          {items.map((item, idx) => (
                            <tr key={`${group}-${item.course_code}-${idx}`} className="hover:bg-muted/30">
                              {gradeCategory === "all" && (
                                <td className="py-3 px-4">
                                  {item.is_minor ? "辅修" : "主修"}
                                </td>
                              )}
                              <td className="py-3 px-4">{item.term}</td>
                              <td className="py-3 px-4 font-mono">{item.course_code}</td>
                              <td className="py-3 px-4">{item.course_name}</td>
                              <td className="py-3 px-4">{item.credit ?? "—"}</td>
                              <td className="py-3 px-4">{item.total_hours ?? "—"}</td>
                              <td className="py-3 px-4">{item.exam_mode ?? "—"}</td>
                              <td className="py-3 px-4">{item.course_nature ?? "—"}</td>
                              <td className="py-3 px-4">{item.course_attr ?? "—"}</td>
                              <td className="py-3 px-4">
                                <div className="flex items-center gap-2">
                                  <Button
                                    size="sm"
                                    variant="outline"
                                    onClick={() => handleEditPlanCourse(item)}
                                  >
                                    编辑
                                  </Button>
                                  <Button
                                    size="sm"
                                    variant="outline"
                                    onClick={() => handleDeletePlanCourse(item)}
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
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
