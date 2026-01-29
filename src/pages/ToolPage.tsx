import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";
import PasswordCrackerTool from "@/components/PasswordCrackerTool";
import GradeTool from "@/components/GradeTool";

export default function ToolPage() {
  const { id } = useParams();
  const navigate = useNavigate();

  if (id === "grades") {
    return <GradeTool />;
  }

  // 教务日期查询工具的专用渲染
  if (id === "password-cracker") {
    return <PasswordCrackerTool />;
  }

  useEffect(() => {
    navigate("/", { replace: true });
  }, [navigate]);

  return null;
}
