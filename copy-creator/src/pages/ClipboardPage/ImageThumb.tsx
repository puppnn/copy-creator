import { useState, useEffect, useRef } from "react";
import { useClipboardStore } from "../../stores/clipboardStore";

const HOVER_PREVIEW_DELAY_MS = 500;

interface ImageThumbProps {
  record: { id: string; content: string };
  onHover: (src: string, rect: DOMRect) => void;
  onLeave: () => void;
  onClick: (e: React.MouseEvent) => void;
}

export function ImageThumb({ record, onHover, onLeave, onClick }: ImageThumbProps) {
  const { getThumbnail, getImageData, thumbnailCache } = useClipboardStore();
  const [loadedSrc, setLoadedSrc] = useState<string | null>(null);
  const [visible, setVisible] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const hoveredRef = useRef(false);
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverSessionRef = useRef(0);
  const cachedSrc = thumbnailCache[record.id] ?? null;
  const src = loadedSrc ?? cachedSrc;

  useEffect(() => {
    if (!visible || cachedSrc) return;
    getThumbnail(record).then((dataUrl) => {
      if (dataUrl) setLoadedSrc(dataUrl);
    });
  }, [cachedSrc, getThumbnail, record, visible]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) setVisible(true);
      },
      { rootMargin: "200px" }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  useEffect(
    () => () => {
      hoveredRef.current = false;
      hoverSessionRef.current += 1;
      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
    },
    [],
  );

  return (
    <div
      ref={ref}
      className="clipboard-card-thumb"
      onMouseEnter={(e) => {
        hoveredRef.current = true;
        const session = ++hoverSessionRef.current;
        const rect = e.currentTarget.getBoundingClientRect();
        let previewSrc = src;
        let delayElapsed = false;

        if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
        hoverTimerRef.current = setTimeout(() => {
          hoverTimerRef.current = null;
          delayElapsed = true;
          if (hoveredRef.current && hoverSessionRef.current === session && previewSrc) {
            onHover(previewSrc, rect);
          }
        }, HOVER_PREVIEW_DELAY_MS);

        getImageData(record).then((fullSrc) => {
          if (!fullSrc || !hoveredRef.current || hoverSessionRef.current !== session) return;
          previewSrc = fullSrc;
          if (delayElapsed) onHover(fullSrc, rect);
        });
      }}
      onMouseLeave={() => {
        hoveredRef.current = false;
        hoverSessionRef.current += 1;
        if (hoverTimerRef.current) {
          clearTimeout(hoverTimerRef.current);
          hoverTimerRef.current = null;
        }
        onLeave();
      }}
      onClick={onClick}
    >
      {src ? (
        <img src={src} alt="" />
      ) : (
        <div className="thumb-spinner" />
      )}
    </div>
  );
}
