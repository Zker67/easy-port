import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import App from "@/App";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
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
      {/* 全应用共享一个 Provider：Radix 据此实现「移到相邻控件立即显示」，
          每处各挂一个会退化成每次都重新等待延迟。 */}
      <TooltipProvider>
        <App />
      </TooltipProvider>
      <Toaster />
    </QueryClientProvider>
  </React.StrictMode>,
);
