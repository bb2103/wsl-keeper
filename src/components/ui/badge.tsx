import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium",
  {
    variants: {
      variant: {
        default: "border-transparent bg-muted text-muted-foreground",
        ok: "border-transparent bg-[color-mix(in_srgb,var(--ok)_16%,transparent)] text-[var(--ok)]",
        warn: "border-transparent bg-[color-mix(in_srgb,var(--warn)_16%,transparent)] text-[var(--warn)]",
        pause: "border-transparent bg-[color-mix(in_srgb,var(--pause)_16%,transparent)] text-[var(--pause)]",
        danger: "border-transparent bg-[color-mix(in_srgb,var(--danger)_16%,transparent)] text-[var(--danger)]",
        idle: "border-transparent bg-muted text-muted-foreground",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
