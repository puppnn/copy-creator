import { useState } from "react";
import FileDownloadRoundedIcon from "@mui/icons-material/FileDownloadRounded";
import FileUploadRoundedIcon from "@mui/icons-material/FileUploadRounded";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

interface TransferResult {
  path: string;
  settings_count: number;
  favorites_count: number;
}

interface DataSectionProps {
  onImported: () => Promise<void>;
}

export function DataSection({ onImported }: DataSectionProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<string>("");
  const [busy, setBusy] = useState(false);

  const runTransfer = async (command: "export_user_data" | "import_user_data") => {
    setBusy(true);
    setStatus("");
    try {
      const result = await invoke<TransferResult>(command);
      if (command === "import_user_data") await onImported();
      setStatus(
        t(command === "export_user_data" ? "settings.exportSuccess" : "settings.importSuccess", {
          settings: result.settings_count,
          favorites: result.favorites_count,
        }),
      );
    } catch (error) {
      if (!String(error).toLowerCase().includes("cancelled")) {
        setStatus(t("settings.transferFailed"));
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="settings-section">
      <div className="settings-section-title">{t("settings.dataTransfer")}</div>
      <div className="settings-card">
        <div className="settings-row settings-transfer-row">
          <button
            className="settings-transfer-btn"
            type="button"
            disabled={busy}
            onClick={() => runTransfer("export_user_data")}
            title={t("settings.exportData")}
          >
            <FileDownloadRoundedIcon />
            <span>{t("settings.exportData")}</span>
          </button>
          <button
            className="settings-transfer-btn"
            type="button"
            disabled={busy}
            onClick={() => runTransfer("import_user_data")}
            title={t("settings.importData")}
          >
            <FileUploadRoundedIcon />
            <span>{t("settings.importData")}</span>
          </button>
        </div>
        {status && <div className="settings-transfer-status">{status}</div>}
      </div>
    </div>
  );
}
