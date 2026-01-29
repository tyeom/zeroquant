import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'
import tailwindcss from '@tailwindcss/vite'

// API URL - Docker에서는 서비스명 사용, 로컬에서는 localhost
const apiUrl = process.env.VITE_API_URL || 'http://localhost:3000'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    tailwindcss(),
    solid(),
  ],
  server: {
    port: 5173,
    host: '0.0.0.0', // Docker 컨테이너에서 외부 접근 허용
    proxy: {
      '/api': {
        target: apiUrl,
        changeOrigin: true,
      },
      '/ws': {
        target: apiUrl.replace('http', 'ws'),
        ws: true,
      },
    },
  },
})
