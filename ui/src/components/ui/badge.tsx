import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

type Variant = "default" | "accent" | "destructive";

const variants: Record<Variant, string> = {
  default: "bg-muted text-muted-foreground",
  accent: "bg-accent/20 text-accent",
  destructive: "bg-destructive/20 text-destructive",
};

export function Badge({
  variant = "default",
  className,
  ...props
}: HTMLAttributes<HTMLSpanElement> & { variant?: Variant }) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium",
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}
