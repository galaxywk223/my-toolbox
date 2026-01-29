import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button"; // 引入我们刚才安装的组件

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // 调用 Rust 后端的 greet 函数
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    // 使用 Tailwind CSS 进行布局 (h-screen 全屏, flex 居中)
    <div className="h-screen flex flex-col items-center justify-center space-y-6 bg-slate-50 dark:bg-slate-950 p-8">
      <div className="text-center space-y-2">
        <h1 className="text-4xl font-extrabold tracking-tight lg:text-5xl text-slate-900 dark:text-slate-50">
          My Toolbox
        </h1>
        <p className="text-slate-500 dark:text-slate-400">
          Tauri + React + Tailwind + Shadcn/ui
        </p>
      </div>

      <div className="flex w-full max-w-sm items-center space-x-2">
        <input
          id="greet-input"
          className="flex h-10 w-full rounded-md border border-slate-300 bg-transparent px-3 py-2 text-sm placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-slate-400 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="输入你的名字..."
        />
        {/* 使用 shadcn 的 Button 组件 */}
        <Button onClick={greet}>打招呼</Button>
      </div>

      {greetMsg && (
        <div className="p-4 rounded-lg bg-white shadow-sm border border-slate-200 text-slate-700">
          {greetMsg}
        </div>
      )}
    </div>
  );
}

export default App;
