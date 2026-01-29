import { useNavigate } from "react-router-dom";
import { Zap, GraduationCap, Clock, ArrowRight, Activity, Calendar } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useState, useEffect, type ComponentType } from "react";
import { Button } from "@/components/ui/button";

type ToolItem = {
  id: string;
  name: string;
  description: string;
  icon: ComponentType<{ className?: string }>;
  color: string;
  bgColor: string;
  border: string;
  disabled?: boolean;
};

type ToolSection = {
  category: string;
  items: ToolItem[];
};

const tools: ToolSection[] = [
  {
    category: "学习",
    items: [
      {
        id: "grades",
        name: "成绩管理",
        description: "多维度的成绩分析与管理工具，支持本地数据存储与隐私保护。",
        icon: GraduationCap,
        color: "text-emerald-500",
        bgColor: "bg-emerald-500/10",
        border: "hover:border-emerald-500/30",
      },
      {
        id: "password-cracker",
        name: "教务系统探针",
        description: "高级教务系统日期节点验证与查询工具 (v2.0)",
        icon: Calendar,
        color: "text-violet-500",
        bgColor: "bg-violet-500/10",
        border: "hover:border-violet-500/30",
      },
    ],
  },
  {
      category: "即将上线",
      items: [
        {
            id: "more",
            name: "更多工具",
            description: "敬请期待更多实用工具上线...",
            icon: Zap,
            color: "text-amber-500",
            bgColor: "bg-amber-500/10",
            border: "hover:border-amber-500/30",
            disabled: true,
        },
        {
            id: "system-status",
            name: "系统状态",
            description: "功能规划中，敬请期待。",
            icon: Activity,
            color: "text-blue-500",
            bgColor: "bg-blue-500/10",
            border: "hover:border-blue-500/30",
            disabled: true,
        },
      ]
  }
];

export default function Dashboard() {
  const navigate = useNavigate();
  const [greeting, setGreeting] = useState("早上好");
  const [time, setTime] = useState(new Date());

  useEffect(() => {
    const hour = new Date().getHours();
    if (hour < 12) setGreeting("早上好");
    else if (hour < 18) setGreeting("下午好");
    else setGreeting("晚上好");

    const timer = setInterval(() => setTime(new Date()), 60000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className="space-y-10 pb-10">
      {/* Hero Section */}
      <div className="relative rounded-3xl bg-gradient-to-br from-primary/10 via-background to-accent/5 p-8 md:p-12 border border-primary/10 overflow-hidden group">
        <div className="absolute top-0 right-0 w-64 h-64 bg-primary/20 rounded-full blur-[100px] -translate-y-1/2 translate-x-1/2 group-hover:bg-primary/30 transition-all duration-1000" />
        
        <div className="relative z-10 space-y-6">
            <div className="space-y-2">
                <div className="flex items-center gap-2 text-primary font-medium text-sm uppercase tracking-wider">
                    <Clock className="w-4 h-4" />
                    <span>{time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                </div>
                <h1 className="text-4xl md:text-5xl font-extrabold tracking-tight">
                {greeting}，<span className="text-primary">同学</span>
                </h1>
                <p className="text-lg text-muted-foreground max-w-xl">
                欢迎回到您的个人工具箱。这里集成了您常用的教务与系统工具，助您高效完成每一项任务。
                </p>
            </div>
            
            <div className="flex gap-4">
                <Button size="lg" className="rounded-full px-8 shadow-lg shadow-primary/20 hover:shadow-primary/40 transition-all hover:scale-105">
                    开始使用 <ArrowRight className="ml-2 w-4 h-4" />
                </Button>
                <Button variant="outline" size="lg" className="rounded-full px-8 hover:bg-white/5 border-primary/20">
                    查看帮助
                </Button>
            </div>
        </div>
      </div>

      {/* Tools Sections */}
      {tools.map((section, sectionIndex) => (
        <div key={section.category} className="space-y-6 animate-in fade-in slide-in-from-bottom-8 duration-700" style={{ animationDelay: `${sectionIndex * 150}ms` }}>
            <div className="flex items-center gap-3">
                <h2 className="text-2xl font-bold tracking-tight">{section.category}</h2>
                <div className="h-px flex-1 bg-border/50" />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
                {section.items.map((tool) => (
                <Card
                    key={tool.id}
                    className={`group relative overflow-hidden border-border/40 bg-card/40 backdrop-blur-md transition-all duration-500 hover:shadow-2xl hover:-translate-y-1 ${tool.disabled ? 'opacity-60 cursor-not-allowed' : 'cursor-pointer hover:border-primary/50'} ${tool.border}`}
                    onClick={() => !tool.disabled && navigate(`/tool/${tool.id}`)}
                >
                    {/* Hover Glow Gradient */}
                    <div className={`absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-500 bg-gradient-to-br from-white/5 to-transparent pointer-events-none`} />
                    
                    <CardHeader className="relative z-10 pb-2">
                        <div className="flex justify-between items-start">
                            <div
                                className={`p-3.5 rounded-2xl transition-transform duration-500 group-hover:scale-110 group-hover:rotate-3 shadow-sm ${tool.bgColor} ${tool.color}`}
                            >
                                <tool.icon className="w-6 h-6" />
                            </div>
                            {!tool.disabled && (
                                <div className="p-2 rounded-full bg-background/50 text-muted-foreground opacity-0 group-hover:opacity-100 -translate-x-2 group-hover:translate-x-0 transition-all duration-300">
                                    <ArrowRight className="w-4 h-4" />
                                </div>
                            )}
                        </div>
                    </CardHeader>

                    <CardContent className="relative z-10 space-y-2 pt-4">
                        <CardTitle className="text-xl font-bold group-hover:text-primary transition-colors">
                            {tool.name}
                        </CardTitle>
                        <CardDescription className="text-sm line-clamp-2 leading-relaxed">
                            {tool.description}
                        </CardDescription>
                    </CardContent>
                    
                    {/* Decorative Bottom Bar */}
                    <div className={`absolute bottom-0 left-0 right-0 h-1 transform scale-x-0 group-hover:scale-x-100 transition-transform duration-500 origin-left ${tool.color.replace('text-', 'bg-')}`} />
                </Card>
                ))}
            </div>
        </div>
      ))}
    </div>
  );
}
