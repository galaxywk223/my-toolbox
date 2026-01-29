import { Palette } from "lucide-react"
import { useTheme, ThemeColor } from "@/components/theme-provider"
import { Button } from "@/components/ui/button"
import { useState, useRef, useEffect } from "react"
import { cn } from "@/lib/utils"

export function ThemeColorSelector() {
  const { themeColor, setThemeColor } = useTheme()
  const [isOpen, setIsOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
        if (ref.current && !ref.current.contains(event.target as Node)) {
            setIsOpen(false)
        }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => {
        document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [ref]);

  const colors: { value: ThemeColor; label: string; colorClass: string }[] = [
    { value: "violet", label: "紫罗兰", colorClass: "bg-purple-600" },
    { value: "ocean", label: "海洋", colorClass: "bg-blue-600" },
    { value: "rose", label: "玫瑰", colorClass: "bg-rose-600" },
    { value: "matcha", label: "抹茶", colorClass: "bg-green-600" },
    { value: "amber", label: "琥珀", colorClass: "bg-orange-500" },
    { value: "slate", label: "岩石", colorClass: "bg-slate-700" },
  ]

  return (
    <div className="relative" ref={ref}>
      <Button
        variant="ghost"
        size="icon"
        onClick={() => setIsOpen(!isOpen)}
        className="rounded-full w-10 h-10 bg-background/50 backdrop-blur-sm border border-border hover:bg-accent hover:text-accent-foreground transition-all"
        title="切换配色主题"
      >
        <Palette className="h-[1.2rem] w-[1.2rem] transition-all" />
        <span className="sr-only">切换配色主题</span>
      </Button>

      {isOpen && (
        <div className="absolute right-0 mt-2 p-3 w-40 rounded-xl border border-border bg-popover shadow-xl animate-in fade-in zoom-in-95 slide-in-from-top-2 z-50">
          <div className="text-xs font-medium text-muted-foreground mb-3 px-1">
            选择主题色
          </div>
          <div className="grid grid-cols-3 gap-3">
            {colors.map((c) => (
              <button
                key={c.value}
                onClick={() => {
                  setThemeColor(c.value)
                  setIsOpen(false)
                }}
                className={cn(
                  "group relative flex h-8 w-8 items-center justify-center rounded-full transition-all hover:scale-110 focus:outline-none",
                  themeColor === c.value 
                    ? "ring-2 ring-primary ring-offset-2 ring-offset-popover scale-110" 
                    : "hover:ring-2 hover:ring-primary/50 hover:ring-offset-1 hover:ring-offset-popover"
                )}
                title={c.label}
              >
                <div className={cn("w-full h-full rounded-full shadow-sm", c.colorClass)} />
                {themeColor === c.value && (
                  <div className="absolute inset-0 flex items-center justify-center">
                    <div className="w-2 h-2 bg-white rounded-full shadow-sm" />
                  </div>
                )}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
