import React from "react";
import { cn } from "@/lib/utils";

interface LogoProps extends React.HTMLAttributes<HTMLDivElement> {
  className?: string;
  size?: number;
}

export const Logo: React.FC<LogoProps> = ({ className, size = 32, ...props }) => {
  return (
    <div
      className={cn(
        "relative flex items-center justify-center rounded-xl bg-gradient-to-br from-primary/20 to-secondary/20 backdrop-blur-sm border border-primary/20 shadow-lg shadow-primary/10 overflow-hidden group transition-all duration-300 hover:shadow-primary/30 hover:scale-105",
        className
      )}
      style={{ width: size, height: size }}
      {...props}
    >
      {/* Background Glow */}
      <div className="absolute inset-0 bg-primary/20 blur-xl opacity-0 group-hover:opacity-100 transition-opacity duration-500" />
      
      {/* SVG Logo */}
      <svg
        width={size * 0.7}
        height={size * 0.7}
        viewBox="0 0 256 256"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className="relative z-10"
      >
        <defs>
          <linearGradient id="logo-gradient-component" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" className="text-primary" stopColor="currentColor" />
            <stop offset="100%" className="text-secondary-foreground" stopColor="currentColor" />
          </linearGradient>
        </defs>
        
        {/* Hexagon Shape */}
        <path
          d="M128 24 L228 80 V176 L128 232 L28 176 V80 Z"
          stroke="url(#logo-gradient-component)"
          strokeWidth="24"
          strokeLinecap="round"
          strokeLinejoin="round"
          fill="none"
          className="drop-shadow-sm"
        />
        
        {/* Inner Y Shape */}
        <path
          d="M128 128 L128 232 M128 128 L228 80 M128 128 L28 80"
          stroke="url(#logo-gradient-component)"
          strokeWidth="24"
          strokeLinecap="round"
          strokeLinejoin="round"
          opacity="0.8"
        />
         {/* Center Dot */}
         <circle cx="128" cy="128" r="20" className="fill-accent animate-pulse" />
      </svg>
    </div>
  );
};
