import * as React from "react";
import { ChevronDown } from "lucide-react";

import { cn } from "@/lib/utils";

function NativeSelect({ className, ...props }: React.ComponentProps<"select">) {
  return (
    <div className="relative w-fit" data-slot="native-select-wrapper">
      <select
        data-slot="native-select"
        className={cn(
          "border-input dark:bg-input/30 h-9 w-full min-w-0 appearance-none rounded-md border bg-transparent py-1 pr-8 pl-3 text-sm shadow-xs transition-[color,box-shadow] outline-none disabled:pointer-events-none disabled:opacity-50",
          "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]",
          "[&>option]:bg-popover [&>option]:text-popover-foreground",
          className,
        )}
        {...props}
      />
      <ChevronDown className="text-muted-foreground pointer-events-none absolute top-1/2 right-2.5 size-4 -translate-y-1/2" />
    </div>
  );
}

export { NativeSelect };
