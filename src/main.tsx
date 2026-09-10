import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import App from "@/App";
import { Toaster } from "@/components/ui/sonner";
import "@/index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // 桌面应用：窗口重新聚焦时不必重复请求，轮询已覆盖
      refetchOnWindowFocus: false,
      retry: false,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
      <Toaster />
    </QueryClientProvider>
  </React.StrictMode>,
);
