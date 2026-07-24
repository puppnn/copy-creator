import { useState, useEffect, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import { useClipboardStore } from "../../stores/clipboardStore";

const HOVER_PREVIEW_DELAY_MS = 300;

interface ImageThumbProps {
  record: { id: string; content: string };
  onHover: (src: string, rect: DOMRect) => void;
  onLeave: () => void;
  onClick: (e: React.MouseEvent) => void;
}

export function ImageThumb({ record, onHover, onLeave, onClick }: ImageThumbProps) {
  const { getThumbnail, getImageData, cachedSrc } = useClipboardStore(
    useShallow((state) => ({
      getThumbnail: state.getThumbnail,
      getImageData: state.getImageData,
      cachedSrc: state.thumbnailCache[record.id] ?? null,
    })),
  );
  const [visible, setVisible] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const hoveredRef = useRef(false);
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverSessionRef = useRef(0);
  const srcRef = useRef(cachedSrc);

  useEffect(() => {
    srcRef.current = cachedSrc;
  }, [cachedSrc]);

  useEffect(() => {
    if (!visible || cachedSrc) return;
    void getThumbnail(record);
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

        if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
        hoverTimerRef.current = setTimeout(() => {
          hoverTimerRef.current = null;
          if (!hoveredRef.current || hoverSessionRef.current !== session) return;

          if (srcRef.current) {
            onHover(srcRef.current, rect);
          }

          getImageData(record).then((fullSrc) => {
            if (!fullSrc || !hoveredRef.current || hoverSessionRef.current !== session) return;
            onHover(fullSrc, rect);
          });
        }, HOVER_PREVIEW_DELAY_MS);
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
      {cachedSrc ? (
        <img src={cachedSrc} alt="" />
      ) : (
        <div className="thumb-spinner" />
      )}
    </div>
  );
}
