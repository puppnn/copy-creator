import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ClipboardPage from "./pages/ClipboardPage";
import PhrasePage from "./pages/PhrasePage";
import TranslationPage from "./pages/TranslationPage";
import SettingsContent from "./components/SettingsContent";
import ApiKeyToast from "./components/ApiKeyToast";
import { useSettingsStore } from "./stores/settingsStore";
import { Icons } from "./components/Icons";
import i18n from "./i18n";

const PANEL_MAP: Record<string, { titleKey: string; component: React.ReactNode }> = {
  clipboard: { titleKey: "tabs.clipboard", component: <ClipboardPage /> },
  phrases: { titleKey: "tabs.phrases", component: <PhrasePage /> },
  translate: { titleKey: "tabs.translate", component: <TranslationPage /> },
};

const NAV_ITEMS = [
  { panelType: "clipboard" },
  { panelType: "phrases" },
  { panelType: "translate" },
] as const;

function App() {
  const { t } = useTranslation();
  const [activePanel, setActivePanel] = useState<string>("clipboard");
  const [unreadCount, setUnreadCount] = useState(0);
  const { themeMode, toggleTheme, loadSettings } = useSettingsStore();
  const [isPinned, setIsPinned] = useState(false);
  const markReadInFlightRef = useRef<Promise<void> | null>(null);

  const markClipboardRead = useCallback(() => {
    if (markReadInFlightRef.current) return markReadInFlightRef.current;
    const request = invoke<void>("mark_clipboard_read")
      .then(() => setUnreadCount(0))
      .catch((e) => console.error("Failed to mark clipboard as read:", e))
      .finally(() => {
        markReadInFlightRef.current = null;
      });
    markReadInFlightRef.current = request;
    return request;
  }, []);

  useEffect(() => {
    const unlisten = listen("show-clipboard", () => {
      setActivePanel("clipboard");
      void markClipboardRead();
    });

    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [markClipboardRead]);

  useEffect(() => {
    let disposed = false;
    const cleanup: Array<() => void> = [];
    invoke<number>("get_clipboard_unread_count")
      .then((count) => {
        if (!disposed) setUnreadCount(count);
      })
      .catch(console.error);
    listen<number>("clipboard-unread-changed", (event) => {
      if (!disposed) setUnreadCount(event.payload);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else cleanup.push(unlisten);
    });

    const currentWindow = getCurrentWindow();
    currentWindow.onFocusChanged(({ payload: focused }) => {
      if (focused && activePanel === "clipboard") markClipboardRead();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else cleanup.push(unlisten);
    });
    Promise.all([currentWindow.isVisible(), currentWindow.isFocused()]).then(
      ([visible, focused]) => {
        if (!disposed && visible && focused && activePanel === "clipboard") {
          markClipboardRead();
        }
      },
    );

    return () => {
      disposed = true;
      cleanup.forEach((unlisten) => unlisten());
    };
  }, [activePanel, markClipboardRead]);

  useEffect(() => {
    loadSettings().then(() => {
      const lang = useSettingsStore.getState().language;
      if (lang && lang !== i18n.language) {
        i18n.changeLanguage(lang);
      }
    });
  }, [loadSettings]);

  const SIDEBAR_MIN = 60;
  const SIDEBAR_MAX = 130;
  const SIDEBAR_DEFAULT = 60;
  const COLLAPSE_THRESHOLD = 80;
  const [sidebarWidth, setSidebarWidth] = useState(SIDEBAR_DEFAULT);
  const [isCollapsed, setIsCollapsed] = useState(true);
  const sidebarRef = useRef<HTMLDivElement>(null);
  const isDragging = useRef(false);
  const dragStartX = useRef(0);
  const dragStartWidth = useRef(0);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", themeMode);
  }, [themeMode]);

  const handleResizeMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    isDragging.current = true;
    dragStartX.current = e.clientX;
    dragStartWidth.current = sidebarWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, [sidebarWidth]);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isDragging.current) return;
      const delta = e.clientX - dragStartX.current;
      const newWidth = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, dragStartWidth.current + delta));
      const el = sidebarRef.current;
      if (el) {
        el.style.width = `${newWidth}px`;
        el.style.minWidth = `${newWidth}px`;
        if (newWidth <= COLLAPSE_THRESHOLD) {
          el.classList.add("collapsed");
        } else {
          el.classList.remove("collapsed");
        }
      }
    };

    const handleMouseUp = () => {
      if (!isDragging.current) return;
      isDragging.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      const el = sidebarRef.current;
      if (el) {
        const finalWidth = parseFloat(el.style.width);
        if (!isNaN(finalWidth)) {
          setSidebarWidth(finalWidth);
          setIsCollapsed(finalWidth <= COLLAPSE_THRESHOLD);
        }
      }
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, []);

  const handleSettingsClick = () => setActivePanel("settings");

  const handleHide = async () => {
    await getCurrentWindow().hide();
  };

  const handleMinimize = async () => {
    await getCurrentWindow().minimize();
  };

  const handleTogglePin = async () => {
    try {
      const next = await invoke<boolean>("toggle_always_on_top");
      setIsPinned(next);
    } catch (e) {
      console.error("Failed to toggle pin:", e);
    }
  };

  const panelInfo = activePanel !== "settings" ? PANEL_MAP[activePanel] : null;
  const isSettingsPanel = activePanel === "settings";

  return (
    <>
    <div className="app-container">
      <div
        ref={sidebarRef}
        className={`sidebar ${isCollapsed ? "collapsed" : ""}`}
        style={{ width: sidebarWidth, minWidth: sidebarWidth }}
      >
        <div className="sidebar-header">
          <img
            className="sidebar-logo"
            src="/logo_top.png"
            alt="logo"
            draggable={false}
          />
          <span className="sidebar-brand">{t("brand.name")}</span>
        </div>

        <div className="sidebar-nav">
          {NAV_ITEMS.map((item) => {
            const iconKey = item.panelType as keyof typeof Icons;
            const titleKey = `tabs.${item.panelType}`;
            const isActive = !isSettingsPanel && activePanel === item.panelType;
            return (
              <button
                key={item.panelType}
                className={`sidebar-nav-item ${isActive ? "active" : ""}`}
                onClick={() => {
                  setActivePanel(item.panelType);
                  if (item.panelType === "clipboard") markClipboardRead();
                }}
                title={t(titleKey)}
              >
                <span className="sidebar-nav-icon">
                  {Icons[iconKey]}
                  {item.panelType === "clipboard" && unreadCount > 0 && (
                    <span className="sidebar-unread-badge">
                      {unreadCount > 99 ? "99+" : unreadCount}
                    </span>
                  )}
                </span>
                <span className="sidebar-nav-label">{t(titleKey)}</span>
              </button>
            );
          })}
        </div>

        <div className="sidebar-footer">
          <button
            className={`sidebar-footer-item ${isSettingsPanel ? "active" : ""}`}
            onClick={handleSettingsClick}
            title={t("settings.title")}
          >
            <span className="sidebar-footer-icon">{Icons.settings}</span>
            <span className="sidebar-footer-label">{t("settings.title")}</span>
          </button>
          <button
            className="sidebar-footer-item"
            onClick={toggleTheme}
            title={themeMode === "light" ? t("settings.dark") : t("settings.light")}
          >
            <span className="sidebar-footer-icon">
              {themeMode === "light" ? Icons.moon : Icons.sun}
            </span>
            <span className="sidebar-footer-label">
              {themeMode === "light" ? t("settings.dark") : t("settings.light")}
            </span>
          </button>
          <button
            className={`sidebar-footer-item ${isPinned ? "active" : ""}`}
            onClick={handleTogglePin}
            title={isPinned ? t("common.unpinWindow") : t("common.pinWindow")}
          >
            <span className="sidebar-footer-icon">
              {isPinned ? Icons.pin : Icons.pinOff}
            </span>
            <span className="sidebar-footer-label">
              {isPinned ? t("common.unpinWindow") : t("common.pinWindow")}
            </span>
          </button>
        </div>

        <div
          className="sidebar-resize-handle"
          onMouseDown={handleResizeMouseDown}
        />
      </div>

      <div className="panel-area">
        <div className="panel-window-header">
          <h3 className="panel-window-title">
            {isSettingsPanel ? t("settings.title") : panelInfo ? t(panelInfo.titleKey) : ""}
          </h3>
          <div className="window-header-actions">
            <button
              className="window-minimize-btn"
              onClick={handleMinimize}
              title={t("common.minimize")}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <line x1="5" y1="12" x2="19" y2="12" />
              </svg>
            </button>
            <button
              className="window-close-btn"
              onClick={handleHide}
              title={t("common.hide")}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        </div>
        <div className="panel-window-body">
          {isSettingsPanel ? (
            <SettingsContent embedded />
          ) : (
            panelInfo?.component
          )}
        </div>
      </div>
    </div>
    <ApiKeyToast />
    </>
  );
}

export default App;
