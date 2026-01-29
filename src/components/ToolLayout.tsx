import { useNavigate } from "react-router-dom";
import { ArrowLeft, Share2, MoreHorizontal } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ToolLayoutProps {
  title: string;
  description?: string;
  children: React.ReactNode;
  actions?: React.ReactNode;
}

export function ToolLayout({ title, description, children, actions }: ToolLayoutProps) {
  const navigate = useNavigate();

  return (
    <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
      {/* Tool Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-6 border-b border-border/40">
        <div className="space-y-1">
            <div className="flex items-center gap-2 mb-2">
                <Button 
                    variant="ghost" 
                    size="sm" 
                    className="h-8 w-8 p-0 rounded-full text-muted-foreground hover:text-foreground -ml-2"
                    onClick={() => navigate(-1)}
                >
                    <ArrowLeft className="w-4 h-4" />
                </Button>
                <span className="text-sm font-medium text-muted-foreground">Back to Dashboard</span>
            </div>
          <h1 className="text-3xl font-bold tracking-tight bg-gradient-to-r from-foreground to-foreground/70 bg-clip-text text-transparent">
            {title}
          </h1>
          {description && (
            <p className="text-lg text-muted-foreground max-w-2xl">
              {description}
            </p>
          )}
        </div>
        
        <div className="flex items-center gap-2">
             {actions}
             <Button variant="outline" size="icon" className="rounded-full">
                <Share2 className="w-4 h-4" />
             </Button>
             <Button variant="ghost" size="icon" className="rounded-full">
                <MoreHorizontal className="w-4 h-4" />
             </Button>
        </div>
      </div>

      {/* Tool Content Container */}
      <div className="bg-card/30 backdrop-blur-sm border border-border/50 rounded-3xl p-6 md:p-8 shadow-sm min-h-[500px]">
        {children}
      </div>
    </div>
  );
}
