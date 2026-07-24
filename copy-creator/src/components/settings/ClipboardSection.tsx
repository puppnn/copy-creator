import { useTranslation } from "react-i18next";

export interface ClipboardStorageStats {
  record_count: number;
  favorite_count: number;
  usage_bytes: number;
  max_history_items: number;
  max_storage_bytes: number;
  over_limit: boolean;
}

interface ClipboardSectionProps {
  maxHistoryItems: number;
  setMaxHistoryItems: (value: number) => void;
  maxStorageMb: number;
  setMaxStorageMb: (value: number) => void;
  notifications: boolean;
  setNotifications: (value: boolean) => void;
  stats: ClipboardStorageStats | null;
}

function formatBytes(bytes: number) {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function ClipboardSection({
  maxHistoryItems,
  setMaxHistoryItems,
  maxStorageMb,
  setMaxStorageMb,
  notifications,
  setNotifications,
  stats,
}: ClipboardSectionProps) {
  const { t } = useTranslation();
  const usagePercent = stats
    ? Math.min(
        100,
        Math.max(
          (stats.record_count / Math.max(maxHistoryItems, 1)) * 100,
          (stats.usage_bytes / Math.max(maxStorageMb * 1024 * 1024, 1)) * 100,
        ),
      )
    : 0;

  return (
    <div className="settings-section">
      <div className="settings-section-title">{t("settings.clipboard")}</div>
      <div className="settings-card">
        <div className="settings-row">
          <div className="settings-row-label">{t("settings.maxHistoryItems")}</div>
          <div className="settings-number-control">
            <input
              className="settings-number-input"
              type="number"
              min={100}
              max={100000}
              step={100}
              value={maxHistoryItems}
              onChange={(event) => setMaxHistoryItems(Number(event.target.value))}
              aria-label={t("settings.maxHistoryItems")}
            />
            <span>{t("settings.items")}</span>
          </div>
        </div>
        <div className="settings-row">
          <div className="settings-row-label">{t("settings.maxStorageSize")}</div>
          <div className="settings-number-control">
            <input
              className="settings-number-input"
              type="number"
              min={50}
              max={100000}
              step={50}
              value={maxStorageMb}
              onChange={(event) => setMaxStorageMb(Number(event.target.value))}
              aria-label={t("settings.maxStorageSize")}
            />
            <span>MB</span>
          </div>
        </div>
        <div className="settings-row">
          <div className="settings-row-label">{t("settings.clipboardNotifications")}</div>
          <button
            className={`toggle-switch ${notifications ? "on" : "off"}`}
            onClick={() => setNotifications(!notifications)}
            type="button"
            title={notifications ? t("common.on") : t("common.off")}
          >
            <span className="toggle-thumb" />
          </button>
        </div>
        {stats && (
          <div className="settings-row vertical settings-usage-row">
            <div className="settings-usage-summary">
              <span>
                {t("settings.storageUsage", {
                  count: stats.record_count,
                  size: formatBytes(stats.usage_bytes),
                })}
              </span>
              <span>{t("settings.favoriteCount", { count: stats.favorite_count })}</span>
            </div>
            <progress className="settings-usage-progress" value={usagePercent} max={100} />
          </div>
        )}
      </div>
    </div>
  );
}
