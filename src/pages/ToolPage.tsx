import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";
import PasswordCrackerTool from "@/components/PasswordCrackerTool";
import GradeTool from "@/components/GradeTool";
import ScheduleTool from "@/components/ScheduleTool";
import ProjectTreeScannerTool from "@/components/ProjectTreeScannerTool";
import TechStackDetectorTool from "@/components/TechStackDetectorTool";

export default function ToolPage() {
  const { id } = useParams();
  const navigate = useNavigate();

  if (id === "grades") {
    return <GradeTool />;
  }

  if (id === "schedule") {
    return <ScheduleTool />;
  }

  // 教务日期查询工具的专用渲染
  if (id === "password-cracker") {
    return <PasswordCrackerTool />;
  }

  if (id === "project-tree") {
    return <ProjectTreeScannerTool />;
  }

  if (id === "tech-stack") {
    return <TechStackDetectorTool />;
  }

  useEffect(() => {
    navigate("/", { replace: true });
  }, [navigate]);

  return null;
}
