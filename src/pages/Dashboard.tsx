import { useNavigate } from "react-router-dom";
import { Lock } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const tools = [
  {
    id: "password-cracker",
    name: "教务日期查询",
    description: "日期格式教务日期查询（内部调试用）",
    icon: Lock,
    color: "text-pink-500",
  },
];

export default function Dashboard() {
  const navigate = useNavigate();

  return (
    <div className="min-h-screen bg-slate-50 dark:bg-slate-950 p-8">
      <div className="max-w-6xl mx-auto space-y-8">
        <div className="space-y-2">
          <h1 className="text-3xl font-bold tracking-tight text-slate-900 dark:text-slate-50">
            工具箱
          </h1>
          <p className="text-slate-500 dark:text-slate-400">
            选择一个工具开始使用
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {tools.map((tool) => (
            <Card
              key={tool.id}
              className="cursor-pointer hover:shadow-md transition-shadow hover:bg-slate-50/50 dark:hover:bg-slate-900/50"
              onClick={() => navigate(`/tool/${tool.id}`)}
            >
              <CardHeader className="flex flex-row items-center gap-4 space-y-0 pb-2">
                <div
                  className={`p-2 rounded-lg bg-slate-100 dark:bg-slate-800 ${tool.color}`}
                >
                  <tool.icon className="w-6 h-6" />
                </div>
                <CardTitle className="text-xl font-medium">
                  {tool.name}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <CardDescription className="text-base mt-2">
                  {tool.description}
                </CardDescription>
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    </div>
  );
}
