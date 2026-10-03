import tailwindcss from "@tailwindcss/vite";

export default defineNuxtConfig({
  compatibilityDate: "2026-10-03",
  ssr: false,
  telemetry: false,
  devtools: { enabled: false },
  css: ["~/assets/main.css", "~/assets/accessibility.css"],
  app: {
    head: {
      title: "CapyDock",
      link: [{ rel: "icon", type: "image/svg+xml", href: "/icon.svg" }],
      htmlAttrs: { lang: "pt-BR" },
      meta: [{ name: "color-scheme", content: "light" }],
    },
  },
  vite: {
    plugins: [tailwindcss()],
    clearScreen: false,
    server: { strictPort: true },
  },
  ignore: ["**/src-tauri/**", "**/crates/**", "**/bin/**"],
});
