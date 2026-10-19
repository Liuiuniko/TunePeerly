// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
import { createContext, useContext, type RefObject } from "react";

export const ScrollContext = createContext<RefObject<HTMLDivElement | null> | null>(null);

export function useScrollRoot(): RefObject<HTMLDivElement | null> {
  const value = useContext(ScrollContext);
  if (!value) throw new Error("ScrollContext missing");
  return value;
}
