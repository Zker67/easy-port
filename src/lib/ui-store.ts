import { create } from "zustand";
import { persist } from "zustand/middleware";

/** 侧栏展开宽度，同时作为拖拽的上限 */
export const SIDEBAR_WIDTH = 176;
/** 收起后的图标态宽度，够放下 36px 图标按钮加两侧留白 */
export const SIDEBAR_COLLAPSED_WIDTH = 52;
/**
 * 拖拽时低于此宽度即吸附为收起态。
 *
 * 取展开与收起宽度的中点偏收起一侧：太靠近展开宽度会导致轻碰就收起，
 * 太靠近收起宽度则要拖到几乎贴边才触发。
 */
export const SIDEBAR_SNAP_WIDTH = 110;

/**
 * 纯客户端 UI 状态。
 *
 * 与 TanStack Query 的分工（见 README 技术选型）：
 * Query 管 Rust 侧的数据，这里只管「用户怎么摆布界面」这类本地偏好。
 * 因此走 localStorage 而不是 `state.json`——它既不需要 Rust 侧知道，
 * 也不该混进隧道配置里。
 */
type UiState = {
  sidebarCollapsed: boolean;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
};

export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      sidebarCollapsed: false,
      setSidebarCollapsed: (sidebarCollapsed) => set({ sidebarCollapsed }),
      toggleSidebar: () =>
        set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
    }),
    { name: "easy-port-ui" },
  ),
);
