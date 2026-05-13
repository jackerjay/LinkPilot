/// <reference types="vite/client" />

// macOS WKWebView drag region — non-standard CSS, no TS type for it.
// The empty export keeps this a module so `declare module` AUGMENTS react's
// types instead of replacing them.
export {};
declare module "react" {
  interface CSSProperties {
    WebkitAppRegion?: "drag" | "no-drag";
  }
}
