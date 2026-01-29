import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";
import PasswordCrackerTool from "@/components/PasswordCrackerTool";

export default function ToolPage() {
  const { id } = useParams();
  const navigate = useNavigate();

  // 教务日期查询工具的专用渲染
  if (id === "password-cracker") {
    return <PasswordCrackerTool />;
  }

  useEffect(() => {
    navigate("/", { replace: true });
  }, [navigate]);

  return null;
}
