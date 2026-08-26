import { useState, useEffect, useCallback, useRef } from 'react'
import { X } from 'lucide-react'
import { cn } from '@/lib/utils'

interface DrawerProps {
  open: boolean
  onClose: () => void
  title: string
  /** Initial width — px number or CSS string like "25%". Default: 288 */
  width?: number | string
  minWidth?: number
  maxWidth?: number
  /** Extra classes on the content card (bg, ring, etc.) */
  className?: string
  /** data-orbit-zone value */
  zone?: string
  /**
   * Milliseconds to wait after open transition before marking content ready.
   * Pass 210 for xterm so ResizeObserver never sees intermediate widths.
   */
  contentDelayMs?: number
  /** ReactNode, or a render prop that receives contentReady. */
  children: React.ReactNode | ((contentReady: boolean) => React.ReactNode)
}

export function Drawer({
  open,
  onClose,
  title,
  width = 288,
  minWidth = 180,
  maxWidth = 640,
  className,
  zone,
  contentDelayMs = 0,
  children,
}: DrawerProps) {
  const [mounted,      setMounted]      = useState(false)
  const [visible,      setVisible]      = useState(false)
  const [contentReady, setContentReady] = useState(false)
  const [currentWidth, setCurrentWidth] = useState<number | string>(width)
  const [dragging,     setDragging]     = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (open) {
      setCurrentWidth(width)
      setMounted(true)
      // Double RAF: paint at width:0 first so the CSS transition runs 0 → full
      requestAnimationFrame(() => requestAnimationFrame(() => setVisible(true)))
      if (contentDelayMs > 0) {
        const t = setTimeout(() => setContentReady(true), contentDelayMs)
        return () => clearTimeout(t)
      } else {
        setContentReady(true)
      }
    } else {
      setVisible(false)
      setContentReady(false)
      const t = setTimeout(() => setMounted(false), 210)
      return () => clearTimeout(t)
    }
  }, [open, width, contentDelayMs])

  const onResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    setDragging(true)

    const startX = e.clientX
    // Measure actual rendered px — works even when currentWidth is a string like "25%"
    const startWidth = containerRef.current?.getBoundingClientRect().width ?? 288

    const onMove = (ev: MouseEvent) => {
      // Drawer is on the right side: dragging left (negative delta) = wider
      const newWidth = Math.max(minWidth, Math.min(maxWidth, startWidth - (ev.clientX - startX)))
      setCurrentWidth(newWidth)
    }
    const onUp = () => {
      setDragging(false)
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
    }
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }, [minWidth, maxWidth])

  if (!mounted) return null

  return (
    <>
      {/* Fullscreen overlay during drag — keeps cursor col-resize if mouse leaves handle */}
      {dragging && (
        <div className="fixed inset-0 z-50 cursor-col-resize" />
      )}

      <div
        ref={containerRef}
        data-orbit-zone={zone}
        className={cn(
          'relative flex shrink-0',
          !dragging && 'transition-all duration-200 ease-in-out',
        )}
        style={{ width: visible ? currentWidth : 0, opacity: visible ? 1 : 0 }}
      >
        {/* Left resize handle — only active when fully visible */}
        {visible && (
          <div
            className="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize z-10"
            onMouseDown={onResizeStart}
          />
        )}

        {/* Content card */}
        <div className={cn('flex-1 flex flex-col overflow-hidden rounded-2xl', className)}>
          {/* Header — h-[36px] matches principal.card tabs */}
          <div className="flex items-center h-[36px] px-3 bg-card border-b border-sidebar-border/60 shrink-0 gap-2">
            <span className="text-xs font-medium text-foreground/55 flex-1 truncate">{title}</span>
            <button
              onClick={onClose}
              className="p-1 rounded-md text-foreground/20 hover:text-foreground/60 hover:bg-foreground/5 transition-colors shrink-0"
              aria-label="Close drawer"
            >
              <X size={12} />
            </button>
          </div>

          {typeof children === 'function' ? children(contentReady) : children}
        </div>
      </div>
    </>
  )
}
