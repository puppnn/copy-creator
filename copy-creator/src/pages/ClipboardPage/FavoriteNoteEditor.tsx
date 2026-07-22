import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

const FAVORITE_NOTE_MAX_LENGTH = 200;

interface Props {
  initialNote: string;
  onSave: (note: string) => Promise<void>;
  onCancel: () => void;
}

export default function FavoriteNoteEditor({ initialNote, onSave, onCancel }: Props) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(initialNote);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
    textareaRef.current?.setSelectionRange(initialNote.length, initialNote.length);
  }, [initialNote]);

  const handleSave = async () => {
    if (saving) return;
    setSaving(true);
    setSaveError(false);
    try {
      await onSave(draft);
      onCancel();
    } catch (error) {
      console.error("Failed to save favorite note:", error);
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="favorite-note-editor"
      onClick={(event) => event.stopPropagation()}
      onContextMenu={(event) => event.stopPropagation()}
    >
      <div className="favorite-note-editor-header">
        <span>{t("clipboard.favoriteNote")}</span>
        <span className="favorite-note-count">
          {draft.length}/{FAVORITE_NOTE_MAX_LENGTH}
        </span>
      </div>
      <textarea
        ref={textareaRef}
        className="favorite-note-textarea"
        value={draft}
        maxLength={FAVORITE_NOTE_MAX_LENGTH}
        placeholder={t("clipboard.favoriteNotePlaceholder")}
        onChange={(event) => {
          setDraft(event.target.value);
          setSaveError(false);
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onCancel();
          } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            void handleSave();
          }
        }}
      />
      {saveError && (
        <span className="favorite-note-error" role="alert">
          {t("clipboard.favoriteNoteSaveFailed")}
        </span>
      )}
      <div className="favorite-note-actions">
        {initialNote && (
          <button
            className="label-panel-chip-btn secondary favorite-note-clear"
            type="button"
            onClick={() => setDraft("")}
            disabled={saving}
          >
            {t("clipboard.clearFavoriteNote")}
          </button>
        )}
        <button
          className="label-panel-chip-btn secondary"
          type="button"
          onClick={onCancel}
          disabled={saving}
        >
          {t("common.cancel")}
        </button>
        <button
          className="label-panel-chip-btn primary"
          type="button"
          onClick={() => void handleSave()}
          disabled={saving}
        >
          {saving ? t("clipboard.favoriteNoteSaving") : t("common.save")}
        </button>
      </div>
    </div>
  );
}
