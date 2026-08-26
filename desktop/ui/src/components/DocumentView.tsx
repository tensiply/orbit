import { useState, useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'
import { ChevronLeft, ChevronRight, ExternalLink, FileText, Loader2 } from 'lucide-react'
import * as pdfjsLib from 'pdfjs-dist'
import type { PDFDocumentProxy, PDFDocumentLoadingTask } from 'pdfjs-dist'
import type { DocEntry } from '../types'

pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
  'pdfjs-dist/build/pdf.worker.mjs',
  import.meta.url,
).href

function fileExt(path: string): string {
  return path.split('.').pop()?.toLowerCase() ?? ''
}

function mimeForExt(ext: string): string | null {
  switch (ext) {
    case 'pdf':        return 'application/pdf'
    case 'html':
    case 'htm':        return 'text/html'
    case 'png':        return 'image/png'
    case 'jpg':
    case 'jpeg':       return 'image/jpeg'
    case 'webp':       return 'image/webp'
    case 'gif':        return 'image/gif'
    case 'csv':        return 'text/plain'
    default:           return null
  }
}

// ── Blob URL hook ─────────────────────────────────────────────────────────────

function useBlobUrl(path: string, mime: string) {
  const [url,   setUrl]   = useState<string | null>(null)
  const [error, setError] = useState(false)

  useEffect(() => {
    let blobUrl: string | null = null
    setUrl(null)
    setError(false)

    invoke<string>('document_read_b64', { path })
      .then((b64) => {
        const binary = atob(b64)
        const bytes  = new Uint8Array(binary.length)
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
        const blob = new Blob([bytes], { type: mime })
        blobUrl = URL.createObjectURL(blob)
        setUrl(blobUrl)
      })
      .catch(() => setError(true))

    return () => { if (blobUrl) URL.revokeObjectURL(blobUrl) }
  }, [path, mime])

  return { url, error }
}

// ── Preview variants ──────────────────────────────────────────────────────────

function Loading() {
  return (
    <div className="flex-1 min-h-0 flex items-center justify-center">
      <Loader2 size={20} className="animate-spin text-foreground/20" />
    </div>
  )
}

function NoPreview({ ext }: { ext: string }) {
  return (
    <div className="flex-1 min-h-0 flex flex-col items-center justify-center gap-2">
      <FileText size={28} className="text-foreground/12" />
      <p className="text-xs text-foreground/30">
        Preview not available for .{ext} files
      </p>
    </div>
  )
}

function FramePreview({ path, mime, title }: { path: string; mime: string; title: string }) {
  const { url, error } = useBlobUrl(path, mime)
  if (error) return <NoPreview ext={fileExt(path)} />
  if (!url)  return <Loading />
  return (
    <iframe
      src={url}
      title={title}
      className="flex-1 min-h-0 w-full border-0"
    />
  )
}

const ZOOM_STEP = 1.25
const ZOOM_MIN  = 0.25
const ZOOM_MAX  = 4.0

function PdfPreview({ path }: { path: string }) {
  const { url, error } = useBlobUrl(path, 'application/pdf')
  const containerRef   = useRef<HTMLDivElement>(null)
  const canvasRefs     = useRef<Map<number, HTMLCanvasElement>>(new Map())
  const docRef         = useRef<PDFDocumentProxy | null>(null)
  const taskRef        = useRef<PDFDocumentLoadingTask | null>(null)
  const fitScaleRef    = useRef(1)
  const [numPages,  setNumPages]  = useState(0)
  const [rendered,  setRendered]  = useState(false)
  const [zoom,      setZoom]      = useState(1.0)   // multiplier over fitScale
  const [page,      setPage]      = useState(1)

  // Load PDF + compute fit-page scale ─────────────────────────────────────────
  useEffect(() => {
    if (!url) return
    let cancelled = false
    setNumPages(0)
    setRendered(false)
    setZoom(1.0)
    setPage(1)
    canvasRefs.current.clear()

    ;(async () => {
      const task = pdfjsLib.getDocument({ url })
      taskRef.current?.destroy()
      taskRef.current = task

      const doc = await task.promise
      if (cancelled) return
      docRef.current = doc

      const page1 = await doc.getPage(1)
      const vp1   = page1.getViewport({ scale: 1 })
      const el    = containerRef.current
      if (!el) return

      const availW = Math.max(el.clientWidth  - 48, 100)
      const availH = Math.max(el.clientHeight - 48, 100)
      fitScaleRef.current = Math.min(availW / vp1.width, availH / vp1.height)

      setNumPages(doc.numPages)
    })().catch(() => {})

    return () => { cancelled = true }
  }, [url])

  // Render pages (re-runs on numPages change or zoom change) ──────────────────
  useEffect(() => {
    if (numPages === 0 || !docRef.current) return
    const doc   = docRef.current
    const scale = fitScaleRef.current * zoom
    let cancelled = false
    setRendered(false)

    ;(async () => {
      for (let i = 1; i <= numPages; i++) {
        if (cancelled) return
        const canvas = canvasRefs.current.get(i)
        if (!canvas) continue
        const pg = await doc.getPage(i)
        const vp = pg.getViewport({ scale })
        canvas.width  = vp.width
        canvas.height = vp.height
        await pg.render({ canvas, viewport: vp }).promise
      }
      if (!cancelled) setRendered(true)
    })().catch(() => {})

    return () => { cancelled = true }
  }, [numPages, zoom])

  // Toolbar actions ────────────────────────────────────────────────────────────
  const zoomIn  = () => setZoom(z => Math.min(z * ZOOM_STEP, ZOOM_MAX))
  const zoomOut = () => setZoom(z => Math.max(z / ZOOM_STEP, ZOOM_MIN))
  const fitPage = () => setZoom(1.0)

  const goTo = (n: number) => {
    const clamped = Math.max(1, Math.min(n, numPages))
    setPage(clamped)
    canvasRefs.current.get(clamped)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

  const displayPct = numPages > 0
    ? Math.round(fitScaleRef.current * zoom * 100)
    : 0

  const isFit = Math.abs(zoom - 1.0) < 0.01

  if (error) return <NoPreview ext="pdf" />

  return (
    <div className="flex flex-col flex-1 min-h-0 overflow-hidden">

      {/* Canvas area */}
      <div
        ref={containerRef}
        className="flex-1 min-h-0 overflow-auto bg-background flex flex-col items-center py-6 gap-4"
      >
        {(!url || numPages === 0) ? (
          <div className="w-full h-full flex items-center justify-center">
            <Loader2 size={20} className="animate-spin text-foreground/20" />
          </div>
        ) : (
          Array.from({ length: numPages }, (_, i) => (
            <canvas
              key={i + 1}
              ref={el => { if (el) canvasRefs.current.set(i + 1, el) }}
              style={{ opacity: rendered ? 1 : 0.01, transition: 'opacity 0.2s ease' }}
              className="shadow-[0_4px_24px_rgba(0,0,0,0.45)] rounded-[2px] shrink-0"
            />
          ))
        )}
      </div>

      {/* Toolbar */}
      <div className="h-8 px-3 flex items-center gap-1 bg-card shrink-0 select-none border-t border-sidebar-border/60">
        {/* Page navigation */}
        <button
          onClick={() => goTo(page - 1)}
          disabled={page <= 1 || numPages === 0}
          className="flex items-center justify-center w-6 h-6 rounded hover:bg-foreground/8 disabled:opacity-25 text-foreground/50 hover:text-foreground/80 transition-colors"
        >
          <ChevronLeft size={13} />
        </button>
        <span className="text-[11px] text-foreground/45 tabular-nums min-w-[56px] text-center">
          {numPages > 0 ? `${page} / ${numPages}` : '—'}
        </span>
        <button
          onClick={() => goTo(page + 1)}
          disabled={page >= numPages || numPages === 0}
          className="flex items-center justify-center w-6 h-6 rounded hover:bg-foreground/8 disabled:opacity-25 text-foreground/50 hover:text-foreground/80 transition-colors"
        >
          <ChevronRight size={13} />
        </button>

        {/* Separator */}
        <span className="w-px h-4 bg-foreground/10 mx-1" />

        {/* Zoom controls */}
        <button
          onClick={zoomOut}
          disabled={zoom <= ZOOM_MIN}
          className="flex items-center justify-center w-6 h-6 rounded text-base leading-none hover:bg-foreground/8 disabled:opacity-25 text-foreground/50 hover:text-foreground/80 transition-colors"
        >
          −
        </button>
        <span className="text-[11px] text-foreground/45 tabular-nums min-w-[38px] text-center">
          {numPages > 0 ? `${displayPct}%` : '—'}
        </span>
        <button
          onClick={zoomIn}
          disabled={zoom >= ZOOM_MAX}
          className="flex items-center justify-center w-6 h-6 rounded text-base leading-none hover:bg-foreground/8 disabled:opacity-25 text-foreground/50 hover:text-foreground/80 transition-colors"
        >
          +
        </button>

        {/* Fit-page toggle */}
        <button
          onClick={fitPage}
          disabled={isFit || numPages === 0}
          className="ml-1 text-[10px] px-1.5 py-0.5 rounded border border-foreground/15 text-foreground/40 hover:text-foreground/70 hover:border-foreground/30 disabled:opacity-30 transition-colors"
        >
          Fit
        </button>
      </div>
    </div>
  )
}

function ImagePreview({ path, mime }: { path: string; mime: string }) {
  const { url, error } = useBlobUrl(path, mime)
  if (error) return <NoPreview ext={fileExt(path)} />
  if (!url)  return <Loading />
  return (
    <div className="flex-1 min-h-0 flex items-center justify-center overflow-hidden bg-document-viewer-bg">
      <img src={url} className="max-w-full max-h-full object-contain" />
    </div>
  )
}

function CsvPreview({ path }: { path: string }) {
  const { url, error } = useBlobUrl(path, 'text/plain')
  const [rows, setRows] = useState<string[][]>([])

  useEffect(() => {
    if (!url) return
    fetch(url)
      .then((r) => r.text())
      .then((text) => {
        const parsed = text
          .trim()
          .split('\n')
          .map((line) => line.split(',').map((c) => c.trim().replace(/^"|"$/g, '')))
        setRows(parsed)
      })
      .catch(() => {})
  }, [url])

  if (error)          return <NoPreview ext="csv" />
  if (!url || !rows.length) return <Loading />

  const header = rows[0]
  const body   = rows.slice(1)

  return (
    <div className="flex-1 min-h-0 overflow-auto p-4 bg-card">
      <table className="text-xs w-full border-collapse">
        <thead>
          <tr>
            {header.map((h, i) => (
              <th
                key={i}
                className="text-left px-2 py-1.5 text-[10px] uppercase tracking-wider text-foreground/40 font-medium border-b border-foreground/8 whitespace-nowrap"
              >
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {body.map((row, ri) => (
            <tr key={ri} className="border-b border-foreground/5 hover:bg-foreground/3">
              {row.map((cell, ci) => (
                <td key={ci} className="px-2 py-1.5 text-foreground/60 whitespace-nowrap">
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function PreviewArea({ doc }: { doc: DocEntry }) {
  const ext  = fileExt(doc.output_path)
  const mime = mimeForExt(ext)

  if (!mime) return <NoPreview ext={ext} />

  if (['png', 'jpg', 'jpeg', 'webp', 'gif'].includes(ext)) {
    return <ImagePreview path={doc.output_path} mime={mime} />
  }
  if (ext === 'pdf') {
    return <PdfPreview path={doc.output_path} />
  }
  if (['html', 'htm'].includes(ext)) {
    return <FramePreview path={doc.output_path} mime={mime!} title={doc.title} />
  }
  if (ext === 'csv') {
    return <CsvPreview path={doc.output_path} />
  }
  return <NoPreview ext={ext} />
}

// ── Header bar ────────────────────────────────────────────────────────────────

function HeaderBar({ doc }: { doc: DocEntry }) {
  const file = doc.output_path.split('/').pop() ?? doc.output_path

  return (
    <div className="flex items-center gap-2 h-8 px-3 shrink-0 border-b border-sidebar-border/60 bg-card">
      <FileText size={13} className="text-foreground/30 shrink-0" />
      <span
        className="text-xs font-medium text-foreground/70 truncate flex-1"
        title={doc.title}
      >
        {doc.title}
      </span>
      <span className="text-[10px] uppercase tracking-wider font-medium px-1.5 py-0.5 rounded bg-foreground/8 text-foreground/40 shrink-0">
        {doc.format}
      </span>
      <span
        className="text-[10px] text-foreground/25 truncate max-w-[140px] hidden sm:block"
        title={file}
      >
        {file}
      </span>
      <button
        onClick={() => void open(doc.output_path)}
        className="flex items-center gap-1 text-[11px] px-2 py-1 rounded bg-foreground/6 hover:bg-foreground/12 text-foreground/45 hover:text-foreground/80 transition-colors shrink-0"
      >
        <ExternalLink size={11} />
        Open
      </button>
    </div>
  )
}

// ── Export ────────────────────────────────────────────────────────────────────

export function DocumentView({ doc }: { doc: DocEntry }) {
  return (
    <div className="flex flex-col h-full overflow-hidden border-x-4 border-card">
      <HeaderBar doc={doc} />
      <PreviewArea doc={doc} />
    </div>
  )
}
