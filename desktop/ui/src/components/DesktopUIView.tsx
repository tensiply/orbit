// Visual reference for the desktop app layout — canonical names for every zone.

function Name({ children }: { children: React.ReactNode }) {
  return (
    <code className="text-[11px] font-mono font-semibold bg-primary/10 border border-primary/20 rounded px-1.5 py-px text-primary/70 shrink-0">
      {children}
    </code>
  )
}

function Mono({ children }: { children: React.ReactNode }) {
  return (
    <code className="text-[10px] font-mono bg-foreground/6 border border-foreground/10 rounded px-1.5 py-px text-foreground/50">
      {children}
    </code>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-3">
      <p className="text-[9px] font-semibold text-foreground/30 uppercase tracking-widest">{title}</p>
      {children}
    </div>
  )
}

function Zone({
  name,
  label,
  sub,
  dim,
  children,
}: {
  name: string
  label: string
  sub?: string
  dim?: boolean
  children?: React.ReactNode
}) {
  return (
    <div className={`border rounded-lg px-3 py-2.5 flex flex-col gap-1.5 ${dim ? 'border-foreground/8 bg-foreground/2' : 'border-foreground/15 bg-foreground/4'}`}>
      <div className="flex items-center gap-2 flex-wrap">
        <Name>{name}</Name>
        <span className={`text-xs font-medium ${dim ? 'text-foreground/40' : 'text-foreground/65'}`}>{label}</span>
      </div>
      {sub && <p className="text-[10px] text-foreground/30 leading-relaxed">{sub}</p>}
      {children && <div className="mt-1 flex flex-col gap-1.5 pl-2 border-l border-foreground/10">{children}</div>}
    </div>
  )
}

export function DesktopUIView() {
  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-8 py-8 flex flex-col gap-10">

        <div>
          <h1 className="text-sm font-semibold text-foreground">UI Map</h1>
          <p className="text-xs text-foreground/40 mt-0.5">
            Nombres canónicos de cada zona de la app. Úsalos para señalar exactamente qué quieres cambiar.
          </p>
        </div>

        {/* ── Referencia rápida ─────────────────────────────────── */}
        <Section title="Referencia rápida — todos los nombres">
          <div className="grid grid-cols-2 gap-x-6 gap-y-1.5">
            {[
              ['orbit.desktop.sidebar',                       'Sidebar completo'],
              ['orbit.desktop.sidebar.header',                '"orbit" + drag region'],
              ['orbit.desktop.sidebar.rail',                  'Columna de iconos'],
              ['orbit.desktop.sidebar.rail.{view}',           'Botón individual del rail'],
              ['orbit.desktop.sidebar.panel',                 'Columna de contenido'],
              ['orbit.desktop.sidebar.panel.header',          'Label + ViewModeToggle'],
              ['orbit.desktop.sidebar.panel.content',         'Lista scrollable'],
              ['orbit.desktop.sidebar.panel.footer',          'Botón New shell / New session'],
              ['orbit.desktop.sidebar.panel.scope-nav',       'Drill-down de scope'],
              ['orbit.desktop.sidebar.panel.session-list',    'Lista de sesiones'],
              ['orbit.desktop.sidebar.panel.session-item',    'Una sesión en la lista'],
              ['orbit.desktop.sidebar.panel.docs',            'Panel Documentation'],
              ['orbit.desktop.principal',                     'Columna principal (derecha)'],
              ['orbit.desktop.principal.titlebar',            'Barra superior'],
              ['orbit.desktop.principal.titlebar.workspace',  'Workspace picker'],
              ['orbit.desktop.principal.titlebar.controls',   'Minimize / Maximize / Close'],
              ['orbit.desktop.principal.card',                'Card central'],
              ['orbit.desktop.principal.card.tabs',           'Barra de pestañas'],
              ['orbit.desktop.principal.card.session-header', 'Breadcrumb de sesión activa'],
              ['orbit.desktop.principal.card.content',        'Área de contenido del tab'],
              ['orbit.desktop.drawer.terminal',               'Terminal drawer (derecha)'],
              ['orbit.desktop.drawer.arch-editor',            'Arch editor drawer (derecha)'],
              ['orbit.desktop.modal.launch-picker',           'Dialog lanzar sesión'],
              ['orbit.desktop.modal.settings-menu',           'Dropdown del botón Settings'],
            ].map(([name, desc]) => (
              <div key={name} className="flex items-start gap-2 py-1 border-b border-foreground/5">
                <Name>{name}</Name>
                <span className="text-[10px] text-foreground/40 leading-tight pt-px">{desc}</span>
              </div>
            ))}
          </div>
        </Section>

        {/* ── Diagrama ──────────────────────────────────────────── */}
        <Section title="Diagrama de layout">
          <div className="border border-foreground/12 rounded-xl p-4 flex gap-2 bg-foreground/2 min-h-[220px]">
            {/* sidebar */}
            <div className="w-[148px] shrink-0 border border-primary/20 rounded-lg p-2 flex flex-col gap-1.5 bg-primary/5">
              <div className="flex items-center gap-1.5 mb-0.5">
                <span className="text-[8px] font-mono font-semibold text-primary/60">sidebar</span>
              </div>
              <div className="border border-foreground/12 rounded px-2 py-1 bg-foreground/3">
                <span className="text-[8px] font-mono text-foreground/40">sidebar.header</span>
              </div>
              <div className="flex gap-1.5 flex-1">
                <div className="w-[30px] shrink-0 border border-foreground/12 rounded px-1 py-1 bg-foreground/3 flex flex-col items-center gap-0.5">
                  <span className="text-[6px] font-mono text-foreground/30 text-center leading-tight">sidebar.rail</span>
                </div>
                <div className="flex-1 border border-foreground/12 rounded px-1.5 py-1 bg-foreground/3 flex flex-col gap-0.5">
                  <span className="text-[6px] font-mono text-foreground/30">sidebar.panel</span>
                  <span className="text-[6px] font-mono text-foreground/20 pl-1">.header</span>
                  <span className="text-[6px] font-mono text-foreground/20 pl-1">.content</span>
                  <span className="text-[6px] font-mono text-foreground/20 pl-1">.footer</span>
                </div>
              </div>
            </div>

            {/* resize divider */}
            <div className="w-px bg-foreground/15 shrink-0 self-stretch rounded" />

            {/* principal */}
            <div className="flex-1 border border-primary/20 rounded-lg p-2 flex flex-col gap-1.5 bg-primary/5">
              <div className="flex items-center gap-1.5 mb-0.5">
                <span className="text-[8px] font-mono font-semibold text-primary/60">principal</span>
              </div>
              <div className="border border-foreground/12 rounded px-2 py-1 bg-foreground/3 flex gap-1 items-center">
                <span className="text-[6px] font-mono text-foreground/30 flex-1">principal.titlebar</span>
                <span className="text-[6px] font-mono text-foreground/20">.workspace</span>
                <span className="text-[6px] font-mono text-foreground/20">.controls</span>
              </div>
              <div className="flex gap-1.5 flex-1">
                {/* card */}
                <div className="flex-1 border border-foreground/15 rounded-lg p-1.5 bg-foreground/3 flex flex-col gap-1">
                  <span className="text-[6px] font-mono text-foreground/35 font-semibold">principal.card</span>
                  <div className="border border-foreground/8 rounded px-1.5 py-0.5 bg-foreground/3">
                    <span className="text-[6px] font-mono text-foreground/25">.tabs</span>
                  </div>
                  <div className="border border-foreground/8 rounded px-1.5 py-0.5 bg-foreground/3">
                    <span className="text-[6px] font-mono text-foreground/25">.session-header</span>
                  </div>
                  <div className="flex-1 border border-foreground/8 rounded px-1.5 py-1 bg-foreground/3">
                    <span className="text-[6px] font-mono text-foreground/25">.content</span>
                  </div>
                </div>
                {/* drawers */}
                <div className="w-[52px] shrink-0 flex flex-col gap-1">
                  <div className="flex-1 border border-foreground/10 rounded px-1 py-1 bg-foreground/3 flex items-center justify-center">
                    <span className="text-[5.5px] font-mono text-foreground/25 text-center leading-tight">drawer.arch-editor</span>
                  </div>
                  <div className="flex-1 border border-foreground/10 rounded px-1 py-1 bg-foreground/3 flex items-center justify-center">
                    <span className="text-[5.5px] font-mono text-foreground/25 text-center leading-tight">drawer.terminal</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Section>

        {/* ── Sidebar ──────────────────────────────────────────── */}
        <Section title="sidebar — Sidebar">
          <Zone name="orbit.desktop.sidebar" label="Sidebar completo" sub="Ancho redimensionable: 180–480 px, default 360 px. Se puede ocultar.">
            <Zone name="orbit.desktop.sidebar.header" label='Barra "orbit"' sub="Texto 'orbit' + región de drag de ventana. Altura h-10." dim />
            <Zone name="orbit.desktop.sidebar.rail" label="Columna de iconos" sub="Ancho fijo 52 px. Navegación principal. Botones de arriba a abajo:">
              <div className="flex flex-col gap-1">
                {[
                  ['orbit.desktop.sidebar.rail.tasks',        'Tasks'],
                  ['orbit.desktop.sidebar.rail.sessions',     'Sessions'],
                  ['orbit.desktop.sidebar.rail.documents',    'Documents'],
                  ['orbit.desktop.sidebar.rail.plans',        'Plans'],
                  ['orbit.desktop.sidebar.rail.plugins',      'Plugins'],
                  ['orbit.desktop.sidebar.rail.mcps',         'MCPs'],
                  ['orbit.desktop.sidebar.rail.activity',     'Activity'],
                  ['orbit.desktop.sidebar.rail.architecture', 'Architecture'],
                  ['orbit.desktop.sidebar.rail.docs',         'Documentation'],
                  ['orbit.desktop.sidebar.rail.settings',     'Settings (abre dropdown)'],
                  ['orbit.desktop.sidebar.rail.profile',      'Profile'],
                ].map(([n, lbl]) => (
                  <div key={n} className="flex items-center gap-2 py-0.5 border-b border-foreground/5">
                    <Name>{n}</Name>
                    <span className="text-[10px] text-foreground/40">{lbl}</span>
                  </div>
                ))}
              </div>
            </Zone>
            <Zone name="orbit.desktop.sidebar.panel" label="Panel de contenido" sub="Columna derecha del sidebar. Cambia según el botón activo del rail.">
              <Zone name="orbit.desktop.sidebar.panel.header" label="Cabecera del panel" sub='Label de la vista activa (ej: "SESSIONS") + ViewModeToggle (All / Scope).' dim />
              <Zone name="orbit.desktop.sidebar.panel.content" label="Contenido scrollable" sub="Varía por vista: session-list, docs, documentos, arquitectura, 'Coming soon'." dim />
              <Zone name="orbit.desktop.sidebar.panel.footer" label="Pie del panel" sub="Solo en Sessions. Botón 'New shell' (modo All) o 'New session in {scope}' (modo Scope)." dim />
              <Zone name="orbit.desktop.sidebar.panel.scope-nav" label="ScopeNavigator" sub="Drill-down workspace → tenant → project → repository. Solo visible en modo Scope." dim />
              <Zone name="orbit.desktop.sidebar.panel.session-list" label="Lista de sesiones" sub="Secciones 'Current' e 'History'. Máximo 10 items combinados." dim />
              <Zone name="orbit.desktop.sidebar.panel.session-item" label="Sesión individual" sub="Punto de estado · label · breadcrumb · engine · tiempo. Click derecho: context menu." dim />
              <Zone name="orbit.desktop.sidebar.panel.docs" label="Panel Documentation" sub="Links a UI Map, UI Kit, Colors." dim />
            </Zone>
          </Zone>
        </Section>

        {/* ── Principal ────────────────────────────────────────── */}
        <Section title="principal — Columna principal">
          <Zone name="orbit.desktop.principal" label="Columna principal" sub="Todo lo que está a la derecha del sidebar.">
            <Zone name="orbit.desktop.principal.titlebar" label="Barra superior" sub="Altura h-10. Región de drag. Alineada a la derecha.">
              <Zone name="orbit.desktop.principal.titlebar.workspace" label="Workspace picker" sub="Botones de acceso rápido a workspaces con sesiones activas + dropdown 'All / workspace'. Shortcut: Alt+W." dim />
              <Zone name="orbit.desktop.principal.titlebar.controls" label="Window controls" sub="Minimize (—) · Maximize (□) · Close (×)." dim />
            </Zone>
            <Zone name="orbit.desktop.principal.card" label="Card central" sub="Fondo rounded-2xl. Ocupa todo el espacio entre el sidebar y los drawers.">
              <Zone name="orbit.desktop.principal.card.tabs" label="Barra de pestañas (TabBar)" sub="Altura h-[36px]. Cada pestaña tiene icono, título truncado y botón ×." dim />
              <Zone name="orbit.desktop.principal.card.session-header" label="Cabecera de sesión (SessionHeader)" sub="Solo visible en tabs de tipo terminal. Icono de engine + breadcrumb de scope completo. h-8." dim />
              <Zone name="orbit.desktop.principal.card.content" label="Área de contenido" sub="Renderiza el componente del tab activo." dim>
                <div className="flex flex-col gap-1">
                  {[
                    ['terminal',      'TerminalPane — xterm.js PTY'],
                    ['settings',      'SettingsView'],
                    ['shortcuts',     'ShortcutsView'],
                    ['uikit',         'UIKitView'],
                    ['colors',        'ColorsView'],
                    ['ui-map',        'DesktopUIView (esta página)'],
                    ['document',      'DocumentView'],
                    ['architecture',  'ArchitectureView'],
                  ].map(([type, comp]) => (
                    <div key={type} className="flex items-center gap-2 py-0.5 border-b border-foreground/5">
                      <Mono>{type}</Mono>
                      <span className="text-[10px] text-foreground/35">{comp}</span>
                    </div>
                  ))}
                </div>
              </Zone>
            </Zone>
            <Zone name="orbit.desktop.drawer.terminal" label="Terminal drawer" sub="Card lateral derecho. Ancho 25% cuando abierto, 0 cuando cerrado (transición 200 ms). Terminal xterm.js independiente." dim />
            <Zone name="orbit.desktop.drawer.arch-editor" label="Architecture editor drawer" sub="Card lateral derecho. Aparece al seleccionar un nodo en ArchitectureView. Formulario de edición de entidad." dim />
          </Zone>
        </Section>

        {/* ── Modales ──────────────────────────────────────────── */}
        <Section title="Modales y overlays">
          <Zone name="orbit.desktop.modal.launch-picker" label="Launch picker" sub='Dialog centrado (260 px). Aparece al lanzar sesión desde orbit.desktop.sidebar.panel.scope-nav. Muestra "Launch session in {scope}" + botones de engine.' />
          <Zone name="orbit.desktop.modal.settings-menu" label="Settings dropdown" sub="DropdownMenu anclado a orbit.desktop.sidebar.rail.settings. Items: Settings (Ctrl+.), Keyboard Shortcuts (Ctrl+,), Switch to Light/Dark." dim />
        </Section>

        {/* ── Archivos ─────────────────────────────────────────── */}
        <Section title="Archivos de referencia">
          <table className="w-full text-left">
            <thead>
              <tr className="border-b border-foreground/10">
                <th className="py-2 pr-4 text-[9px] font-semibold uppercase tracking-widest text-foreground/30">Nombre(s)</th>
                <th className="py-2 pr-4 text-[9px] font-semibold uppercase tracking-widest text-foreground/30">Archivo</th>
              </tr>
            </thead>
            <tbody>
              {[
                [['orbit.desktop.sidebar', 'orbit.desktop.sidebar.rail', 'orbit.desktop.sidebar.panel', 'orbit.desktop.sidebar.panel.*'],        'components/Sidebar.tsx'],
                [['orbit.desktop.principal.titlebar', 'orbit.desktop.principal.titlebar.workspace', 'orbit.desktop.principal.titlebar.controls'], 'components/TitleBar.tsx'],
                [['orbit.desktop.principal.card.tabs'],                                                 'components/TabBar.tsx'],
                [['orbit.desktop.principal.card.session-header'],                                      'components/SessionHeader.tsx'],
                [['orbit.desktop.principal.card.content (terminal)'],                                  'components/Terminal.tsx'],
                [['orbit.desktop.drawer.terminal'],                                                     'components/TerminalDrawer.tsx'],
                [['orbit.desktop.drawer.arch-editor'],                                                  'components/ArchEditDrawer.tsx'],
                [['orbit.desktop.modal.launch-picker'],                                                 'components/LaunchPickerModal.tsx'],
                [['Layout raíz, ResizeHandles'],                                          'App.tsx'],
                [['Tab types, NavView'],                                                  'types/index.ts'],
                [['openUIMap, openUIKit, openColors, etc.'],                              'store/slices/tabs.ts'],
                [['Colores CSS (--background, --foreground, …)'],                        'index.css'],
              ].map(([names, file]) => (
                <tr key={file as string} className="border-b border-foreground/5">
                  <td className="py-2 pr-4 align-top">
                    <div className="flex flex-wrap gap-1">
                      {(names as string[]).map((n) => <Mono key={n}>{n}</Mono>)}
                    </div>
                  </td>
                  <td className="py-2 text-[10px] text-foreground/40 font-mono">{file as string}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>

      </div>
    </div>
  )
}
