import { useState, useCallback, useRef } from 'react'

export function useSidebarResize(defaultWidth: number, min: number, max: number) {
  const [width, setWidth] = useState(defaultWidth)
  const widthRef = useRef(width)
  widthRef.current = width

  const onResizeStart = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      const startX = e.clientX
      const startWidth = widthRef.current

      const onMove = (ev: MouseEvent) => {
        setWidth(Math.max(min, Math.min(max, startWidth + ev.clientX - startX)))
      }
      const onUp = () => {
        document.removeEventListener('mousemove', onMove)
        document.removeEventListener('mouseup', onUp)
      }

      document.addEventListener('mousemove', onMove)
      document.addEventListener('mouseup', onUp)
    },
    [min, max],
  )

  return { width, onResizeStart }
}
