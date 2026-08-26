import * as React from "react"
import { cn } from "@/lib/utils"

function Kbd({ className, ...props }: React.HTMLAttributes<HTMLElement>) {
  return (
    <kbd
      className={cn(
        "inline-flex items-center px-1.5 py-0.5 rounded border border-foreground/20 font-mono text-[10px] text-foreground/55 leading-none select-none",
        className
      )}
      {...props}
    />
  )
}

export { Kbd }
