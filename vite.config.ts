import { defineConfig } from 'vite';

export default defineConfig({
  server: { host: '0.0.0.0', port: 3000, strictPort: true },
  preview: { host: '0.0.0.0', port: 3000, strictPort: true },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) return undefined;
          if (id.includes('@codemirror') || id.includes('@lezer') || id.includes('/codemirror/')) return 'editor-vendor';
          if (id.includes('/lucide-react/')) return 'icons';
          if (/\/(react|react-dom|scheduler)\//.test(id)) return 'react-vendor';
          return undefined;
        },
      },
    },
  },
});
