import { BrowserRouter, Routes, Route } from "react-router-dom";
import Dashboard from "@/pages/Dashboard";
import ToolPage from "@/pages/ToolPage";

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/tool/:id" element={<ToolPage />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
