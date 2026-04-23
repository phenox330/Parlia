import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?:
    | "primary"
    | "primary-soft"
    | "secondary"
    | "danger"
    | "danger-ghost"
    | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { children, className = "", variant = "primary", size = "md", ...props },
    ref,
  ) => {
    const baseClasses =
      "font-medium rounded-lg border focus:outline-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-background motion-safe:transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer";

    const variantClasses = {
      primary:
        "text-white bg-background-ui border-background-ui hover:bg-background-ui/80 hover:border-background-ui/80 focus-visible:ring-background-ui",
      "primary-soft":
        "text-text bg-logo-primary/20 border-transparent hover:bg-logo-primary/30 focus-visible:ring-logo-primary",
      secondary:
        "text-text bg-mid-gray/10 border-border hover:bg-background-ui/30 hover:border-logo-primary focus-visible:ring-logo-primary",
      danger:
        "text-white bg-red-600 border-border hover:bg-red-700 hover:border-red-700 focus-visible:ring-red-500",
      "danger-ghost":
        "text-red-400 border-transparent hover:text-red-300 hover:bg-red-500/10 focus-visible:ring-red-500",
      ghost:
        "text-current border-transparent hover:bg-mid-gray/10 hover:border-logo-primary focus-visible:ring-logo-primary",
    };

    const sizeClasses = {
      sm: "px-2 py-1 text-xs",
      md: "px-4 py-[5px] text-sm",
      lg: "px-4 py-2 text-base",
    };

    return (
      <button
        ref={ref}
        className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
        {...props}
      >
        {children}
      </button>
    );
  },
);
Button.displayName = "Button";
