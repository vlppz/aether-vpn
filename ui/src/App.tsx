import { useEffect } from "react";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { Toaster } from "sonner";
import { RefreshCw } from "lucide-react";
import { useApp, type Page } from "./store";
import { bootstrap } from "./store/actions";
import { useThemeEffect, useIsDark } from "./hooks/useTheme";
import { useGlobalShortcuts } from "./hooks/useGlobalShortcuts";
import { useDropImport } from "./hooks/useDropImport";
import { useSidebarCollapsed } from "./hooks/useSidebar";
import { useT } from "./i18n";
import { TitleBar } from "./components/TitleBar";
import { Sidebar } from "./components/Sidebar";
import { CommandPalette } from "./components/CommandPalette";
import { ImportDialog } from "./components/ImportDialog";
import { ServerPicker } from "./components/ServerPicker";
import { DropOverlay } from "./components/DropOverlay";
import { Logo } from "./components/Logo";
import { Button } from "./components/ui/Button";
import { Home } from "./pages/Home";
import { Servers } from "./pages/Servers";
import { Subscriptions } from "./pages/Subscriptions";
import { Routing } from "./pages/Routing";
import { Logs } from "./pages/Logs";
import { Settings } from "./pages/Settings";

const PAGE_COMPONENTS: Record<Page, () => React.JSX.Element> = {
  home: Home,
  servers: Servers,
  subscriptions: Subscriptions,
  routing: Routing,
  logs: Logs,
  settings: Settings,
};

export default function App() {
  const ready = useApp((s) => s.ready);
  const page = useApp((s) => s.page);
  const dark = useIsDark();
  const dragging = useDropImport();
  const [collapsed, toggleSidebar] = useSidebarCollapsed();
  useThemeEffect();
  useGlobalShortcuts();

  useEffect(() => {
    void bootstrap();
  }, []);

  const PageView = PAGE_COMPONENTS[page];

  return (
    <MotionConfig reducedMotion="user" transition={{ type: "tween", duration: 0.16, ease: "easeOut" }}>
      {ready ? (
        <div className="flex h-full">
          <Sidebar collapsed={collapsed} onToggle={toggleSidebar} />
          <div className="flex min-w-0 flex-1 flex-col">
            <TitleBar sidebarCollapsed={collapsed} />
            <main className="relative min-h-0 flex-1">
              <AnimatePresence mode="wait" initial={false}>
                <motion.div
                  key={page}
                  className="absolute inset-0 px-6 pb-5"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.12 }}
                >
                  <PageView />
                </motion.div>
              </AnimatePresence>
            </main>
          </div>
        </div>
      ) : (
        <Splash />
      )}
      <CommandPalette />
      <ImportDialog />
      <ServerPicker />
      <DropOverlay visible={dragging} />
      <Toaster
        theme={dark ? "dark" : "light"}
        position="bottom-right"
        offset={20}
        gap={8}
        visibleToasts={4}
        toastOptions={{
          style: {
            background: "var(--raised)",
            border: "1px solid var(--line-strong)",
            color: "var(--ink)",
            fontSize: "13px",
          },
        }}
      />
    </MotionConfig>
  );
}

function Splash() {
  const t = useT();
  const bootError = useApp((s) => s.bootError);
  return (
    <div data-tauri-drag-region className="grid h-full place-items-center">
      <div className="flex animate-[fade-in_.4s_ease-out] flex-col items-center gap-4 text-center text-ink">
        <div className="flex items-center gap-2.5">
          <Logo size={26} />
          <span className="font-serif text-[34px] leading-none">Aether</span>
        </div>
        {bootError ? (
          <>
            <div className="text-[13.5px]">{t("boot.error")}</div>
            <code data-selectable className="max-w-md rounded-md border border-line px-3 py-2 font-mono text-[12px] text-danger">
              {bootError}
            </code>
            <Button variant="secondary" icon={<RefreshCw />} onClick={() => void bootstrap()}>
              {t("common.retry")}
            </Button>
          </>
        ) : (
          <div className="animate-[breathe_1.6s_ease-in-out_infinite] text-[12.5px] text-ink-3">{t("common.loading")}</div>
        )}
      </div>
    </div>
  );
}
