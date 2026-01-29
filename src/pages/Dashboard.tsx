import { useNavigate } from "react-router-dom";
import { Lock, Zap, Cpu, GraduationCap } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const tools = [
  {
    id: "grades",
    name: "成绩查询",
    description: "登录教务系统并同步最新成绩",
    icon: GraduationCap,
    color: "text-emerald-500",
    bgColor: "bg-emerald-500/10",
  },
  {
    id: "password-cracker",
    name: "教务日期查询",
    description: "日期格式教务日期查询（内部调试用）",
    icon: Lock,
    color: "text-primary",
    bgColor: "bg-primary/10",
  },
  // Placeholder for future tools to show grid layout
  {
    id: "coming-soon-1",
    name: "更多工具",
    description: "敬请期待...",
    icon: Zap,
    color: "text-muted-foreground",
    bgColor: "bg-muted",
  },
  {
      id: "coming-soon-2",
      name: "系统状态",
      description: "查看系统运行状态",
      icon: Cpu,
      color: "text-muted-foreground",
      bgColor: "bg-muted",
    },
];

export default function Dashboard() {
  const navigate = useNavigate();

  return (
    <div className="space-y-8">
      <div className="space-y-2">
        <h1 className="text-4xl font-extrabold tracking-tight lg:text-5xl bg-gradient-to-r from-primary to-accent bg-clip-text text-transparent animate-in fade-in slide-in-from-left-4 duration-700">
          工具箱
        </h1>
        <p className="text-xl text-muted-foreground max-w-2xl">
          欢迎使用大学生必备工具集合，选择一个工具开始使用。
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
        {tools.map((tool, index) => (
          <Card
            key={tool.id}
            className={`group cursor-pointer border-border/50 hover:border-primary/50 overflow-hidden relative ${tool.id.startsWith('coming') ? 'opacity-60 cursor-not-allowed' : ''}`}
            onClick={() => !tool.id.startsWith('coming') && navigate(`/tool/${tool.id}`)}
            style={{ animationDelay: `${index * 100}ms` }}
          >
             {/* Hover Glow Effect */}
            <div className="absolute inset-0 bg-gradient-to-br from-primary/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-500" />
            
            <CardHeader className="flex flex-row items-center gap-4 space-y-0 pb-2 relative z-10">
              <div
                className={`p-3 rounded-xl transition-transform duration-300 group-hover:scale-110 ${tool.bgColor} ${tool.color}`}
              >
                <tool.icon className="w-6 h-6" />
              </div>
              <CardTitle className="text-xl font-bold">
                {tool.name}
              </CardTitle>
            </CardHeader>
            <CardContent className="relative z-10">
              <CardDescription className="text-base mt-2 line-clamp-2">
                {tool.description}
              </CardDescription>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
