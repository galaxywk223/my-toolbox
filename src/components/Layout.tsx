import { Outlet, Link, useLocation } from "react-router-dom";
import { ThemeToggle } from "./ThemeToggle";
import { ThemeColorSelector } from "./ThemeColorSelector";
import { Github } from "lucide-react";
import { Button } from "./ui/button";
import { Logo } from "./Logo";

export default function Layout() {
  const location = useLocation();

  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col selection:bg-primary/30">
      {/* Background Ambience */}
      <div className="fixed inset-0 pointer-events-none z-[-1] overflow-hidden">
        <div className="absolute top-[-20%] left-[-10%] w-[50%] h-[50%] bg-primary/5 rounded-full blur-[120px]" />
        <div className="absolute bottom-[-20%] right-[-10%] w-[50%] h-[50%] bg-secondary/5 rounded-full blur-[120px]" />
      </div>

      {/* Top Navigation Bar */}
      <header className="sticky top-0 z-50 w-full border-b border-border/40 bg-background/80 backdrop-blur-xl supports-[backdrop-filter]:bg-background/60">
        <div className="container mx-auto px-4 h-16 flex items-center justify-between">
          {/* Logo & Brand */}
          <Link to="/" className="flex items-center gap-2 group">
            <div className="relative">
               <div className="absolute inset-0 bg-primary/50 blur-md rounded-full opacity-0 group-hover:opacity-100 transition-opacity duration-500" />
               <Logo size={32} />
            </div>
            <span className="font-bold text-lg tracking-tight">Toolbox</span>
          </Link>

          {/* Center Navigation (Optional - currently pointing to Dashboard) */}
          <nav className="hidden md:flex items-center gap-6">
            <Link 
                to="/" 
                className={`text-sm font-medium transition-colors hover:text-primary ${location.pathname === "/" ? "text-primary" : "text-muted-foreground"}`}
            >
                总览
            </Link>
            {/* Add more top-level links here if needed */}
          </nav>

          {/* Right Actions */}
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="icon" className="text-muted-foreground hover:text-foreground" asChild>
                <a href="https://github.com" target="_blank" rel="noreferrer">
                    <Github className="w-5 h-5" />
                </a>
            </Button>
            <ThemeColorSelector />
            <ThemeToggle />
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1 container mx-auto px-4 py-8 max-w-7xl animate-in fade-in slide-in-from-bottom-4 duration-500">
        <Outlet />
      </main>

      {/* Simple Footer */}
      <footer className="border-t border-border/40 py-6 md:py-0">
        <div className="container flex flex-col items-center justify-between gap-4 md:h-16 md:flex-row px-4">
          <p className="text-center text-sm leading-loose text-muted-foreground md:text-left">
            使用 Tauri + React + Vite 构建。
          </p>
        </div>
      </footer>
    </div>
  );
}
