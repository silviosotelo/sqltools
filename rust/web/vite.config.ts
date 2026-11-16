import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Configuración mínima de Vite. El diseño visual no se define en esta fase.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2021",
  },
});
