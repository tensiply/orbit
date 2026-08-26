import { useState, useEffect } from 'react'
import { X } from 'lucide-react'
import { cn } from '@/lib/utils'

interface DrawerProps {
  open: boolean
  onClose: () => void
  title: string
  /** px number or CSS string like "25%". Default: 288 */
  width?: number | string
  /** Extra classes on the outer container (bg, ring, etc.) */
  className?: string
  /** data-orbit-zone value */
  zone?: string
  /**
   * Milliseconds to wait after the open transition before marking content
   * as ready. Pass 210 for xterm so ResizeObserver never sees intermediate
   * widths. Default: 0 (children render immediately).
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
  className,
  zone,
  contentDelayMs = 0,
  children,
}: DrawerProps) {
  const [mounted,      setMounted]      = useState(false)
  const [visible,      setVisible]      = useState(false)
  const [contentReady, setContentReady] = useState(false)

  useEffect(() => {
    if (open) {
      setMounted(true)
      // Double RAF: paint at width:0 first so the CSS transition runs from 0 → full
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
  }, [open, contentDelayMs])

  // Not in DOM when fully closed → no flex-row slot consumed
  if (!mounted) return null

  return (
    <div
      data-orbit-zone={zone}
      className={cn(
        'flex flex-col shrink-0 overflow-hidden rounded-2xl transition-all duration-200 ease-in-out',
        className,
      )}
      style={{ width: visible ? width : 0, opacity: visible ? 1 : 0 }}
    >
      {/* Header — matches principal.card tabs height */}
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
  )
}
