import { Outlet, Link, useLocation } from "react-router-dom";
import { ThemeToggle } from "./ThemeToggle";
import { LayoutDashboard, Menu } from "lucide-react";
import { useState } from "react";
import { Button } from "./ui/button";
import { Logo } from "./Logo";

export default function Layout() {
  const location = useLocation();
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);

  const navItems = [
    { icon: LayoutDashboard, label: "Dashboard", path: "/" },
    // { icon: Box, label: "Tools", path: "/tools" }, // Example
  ];

  return (
    <div className="min-h-screen bg-background text-foreground flex overflow-hidden">
      {/* Sidebar - Desktop */}
      <aside className="hidden md:flex w-64 flex-col border-r border-border/40 bg-card/50 backdrop-blur-xl fixed inset-y-0 z-50">
        <div className="p-6 flex items-center gap-3">
          <Logo size={40} />
          <span className="font-bold text-xl tracking-tight">Toolbox</span>
        </div>

        <nav className="flex-1 px-4 space-y-2 mt-4">
          {navItems.map((item) => (
            <Link
              key={item.path}
              to={item.path}
              className={`flex items-center gap-3 px-4 py-3 rounded-xl transition-all duration-200 group ${
                location.pathname === item.path
                  ? "bg-primary text-primary-foreground shadow-md shadow-primary/20"
                  : "hover:bg-accent hover:text-accent-foreground text-muted-foreground"
              }`}
            >
              <item.icon className={`w-5 h-5 ${location.pathname === item.path ? "animate-pulse" : ""}`} />
              <span className="font-medium">{item.label}</span>
            </Link>
          ))}
        </nav>

        <div className="p-4 border-t border-border/40">
          <div className="flex items-center justify-between px-4 py-3 rounded-xl bg-secondary/50 border border-border/50">
            <span className="text-sm font-medium">Dark Mode</span>
            <ThemeToggle />
          </div>
        </div>
      </aside>

      {/* Mobile Header */}
      <div className="md:hidden fixed top-0 left-0 right-0 h-16 bg-background/80 backdrop-blur-md border-b border-border/40 z-50 flex items-center justify-between px-4">
        <div className="flex items-center gap-2">
          <Logo size={32} />
          <span className="font-bold text-lg">Toolbox</span>
        </div>
        <div className="flex items-center gap-2">
            <ThemeToggle />
            <Button variant="ghost" size="icon" onClick={() => setIsSidebarOpen(!isSidebarOpen)}>
                <Menu className="w-6 h-6" />
            </Button>
        </div>
      </div>
      
       {/* Mobile Sidebar Overlay */}
      {isSidebarOpen && (
        <div className="md:hidden fixed inset-0 z-50 bg-black/50 backdrop-blur-sm" onClick={() => setIsSidebarOpen(false)}>
            <div className="absolute right-0 top-0 bottom-0 w-64 bg-card border-l border-border p-4 flex flex-col" onClick={e => e.stopPropagation()}>
                <div className="flex items-center justify-between mb-8">
                     <span className="font-bold text-xl">Menu</span>
                     <Button variant="ghost" size="icon" onClick={() => setIsSidebarOpen(false)}>
                         <Menu className="w-6 h-6" />
                     </Button>
                </div>
                 <nav className="flex-1 space-y-2">
                  {navItems.map((item) => (
                    <Link
                      key={item.path}
                      to={item.path}
                      onClick={() => setIsSidebarOpen(false)}
                      className={`flex items-center gap-3 px-4 py-3 rounded-xl transition-all duration-200 ${
                        location.pathname === item.path
                          ? "bg-primary text-primary-foreground shadow-md shadow-primary/20"
                          : "hover:bg-accent hover:text-accent-foreground text-muted-foreground"
                      }`}
                    >
                      <item.icon className="w-5 h-5" />
                      <span className="font-medium">{item.label}</span>
                    </Link>
                  ))}
                </nav>
            </div>
        </div>
      )}

      {/* Main Content */}
      <main className="flex-1 md:pl-64 pt-16 md:pt-0 min-h-screen relative">
        {/* Background Gradients/Blobs for modern feel */}
        <div className="fixed inset-0 pointer-events-none z-[-1] overflow-hidden">
            <div className="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-primary/5 rounded-full blur-3xl animate-blob" />
            <div className="absolute bottom-[-10%] right-[-10%] w-[40%] h-[40%] bg-secondary/10 rounded-full blur-3xl animate-blob animation-delay-2000" />
             <div className="absolute top-[20%] right-[20%] w-[20%] h-[20%] bg-accent/5 rounded-full blur-3xl animate-blob animation-delay-4000" />
        </div>
        
        <div className="container mx-auto p-6 md:p-8 max-w-7xl animate-in fade-in slide-in-from-bottom-4 duration-500">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
