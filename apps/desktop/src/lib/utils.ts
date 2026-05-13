// shadcn/ui's standard className merger: clsx for conditional class
// composition + tailwind-merge for resolving conflicting utilities so
// e.g. `cn("p-2", "p-4")` correctly keeps just "p-4".

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
