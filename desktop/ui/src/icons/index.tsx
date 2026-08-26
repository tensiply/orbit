export type IconProps = { size?: number }

const defaults = {
  fill: 'none',
  stroke: 'currentColor',
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
  viewBox: '0 0 24 24',
}

// ── Navigation ────────────────────────────────────────────────────────────────

export function TerminalIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </svg>
  )
}

export function TasksIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <polyline points="9 12 11 14 15 10" />
    </svg>
  )
}

export function PlansIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="8" y1="13" x2="16" y2="13" />
      <line x1="8" y1="17" x2="16" y2="17" />
    </svg>
  )
}

export function PluginsIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <rect x="2" y="2" width="9" height="9" rx="1" />
      <rect x="13" y="2" width="9" height="9" rx="1" />
      <rect x="2" y="13" width="9" height="9" rx="1" />
      <rect x="13" y="13" width="9" height="9" rx="1" />
    </svg>
  )
}

export function McpsIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <rect x="2" y="3" width="20" height="7" rx="1" />
      <rect x="2" y="14" width="20" height="7" rx="1" />
      <circle cx="6" cy="6.5" r="0.5" fill="currentColor" />
      <circle cx="6" cy="17.5" r="0.5" fill="currentColor" />
    </svg>
  )
}

export function ActivityIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
    </svg>
  )
}

export function ProfileIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
      <circle cx="12" cy="7" r="4" />
    </svg>
  )
}

export function DocsIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
    </svg>
  )
}

// ── Settings / Actions ────────────────────────────────────────────────────────

export function SettingsIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  )
}

export function KeyboardIcon({ size = 13 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <rect x="2" y="6" width="20" height="12" rx="2" />
      <line x1="6" y1="10" x2="6" y2="10" strokeWidth="2.5" strokeLinecap="round" />
      <line x1="10" y1="10" x2="10" y2="10" strokeWidth="2.5" strokeLinecap="round" />
      <line x1="14" y1="10" x2="14" y2="10" strokeWidth="2.5" strokeLinecap="round" />
      <line x1="18" y1="10" x2="18" y2="10" strokeWidth="2.5" strokeLinecap="round" />
      <line x1="6" y1="14" x2="18" y2="14" strokeWidth="2.5" strokeLinecap="round" />
    </svg>
  )
}

export function ChevronRightIcon({ size = 11 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="2">
      <polyline points="9 18 15 12 9 6" />
    </svg>
  )
}

export function PencilIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
    </svg>
  )
}

export function TrashIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <polyline points="3 6 5 6 21 6" />
      <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
      <path d="M10 11v6M14 11v6" />
      <path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" />
    </svg>
  )
}

export function PlusIcon({ size = 13 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="2">
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
  )
}

export function ResetIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <polyline points="1 4 1 10 7 10" />
      <path d="M3.51 15a9 9 0 1 0 .49-4.95" />
    </svg>
  )
}

export function SearchIcon({ size = 13 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="1.75">
      <circle cx="11" cy="11" r="8" />
      <line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>
  )
}

// ── Engine icons ──────────────────────────────────────────────────────────────

export function ClaudeEngineIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 110 110" fill="currentColor" stroke="none">
      <path d="M21.5640454,73.1229824 L43.18374,60.9914679 L43.5454595,59.934134 L43.18374,59.3498179 L42.1264062,59.3498179 L38.5092115,59.1272213 L26.1551003,58.7933264 L15.442639,58.3481333 L5.0640726,57.7916418 L2.44856258,57.2351503 L0,54.0074996 L0.250421173,52.3936743 L2.44856258,50.9189718 L5.59273953,51.1972175 L12.5488832,51.6702353 L22.9830987,52.3936743 L30.5513831,52.8388675 L41.7646867,54.0074996 L43.5454595,54.0074996 L43.7958807,53.2840606 L43.18374,52.8388675 L42.7107222,52.3936743 L31.9147872,45.0758111 L20.2284658,37.3405793 L14.1070594,32.8886474 L10.795935,30.6348568 L9.12646052,28.5201891 L8.40302157,23.9013097 L11.4080756,20.5901853 L15.442639,20.8684311 L16.4721483,21.1466768 L20.5623607,24.2908538 L29.2992772,31.0522254 L40.7073529,39.455247 L42.3768273,40.8464757 L43.0446171,40.373458 L43.1280909,40.0395631 L42.3768273,38.7874572 L36.1719472,27.5741536 L29.5496984,16.1660779 L26.6002935,11.4359002 L25.8212054,8.5977936 C25.5429596,7.42916146 25.3481876,6.45530134 25.3481876,5.25884463 L28.7706103,0.612140645 L30.6626814,0 L35.2259116,0.612140645 L37.1458073,2.28161513 L39.9839139,8.76474105 L44.5749688,18.97636 L51.6980599,32.8608228 L53.784903,36.9788598 L54.897886,40.7908266 L55.3152546,41.9594587 L56.0386935,41.9594587 L56.0386935,41.2916689 L56.6230096,33.4729634 L57.708168,23.8734851 L58.7655019,11.5193739 L59.1272213,8.0413021 L60.852345,3.86761589 L64.2747677,1.61382534 L66.9459269,2.89375577 L69.1440683,6.03793272 L68.8379979,8.06912668 L67.5302429,16.555622 L64.970382,29.8557687 L63.3009076,38.7596326 L64.2747677,38.7596326 L65.3877507,37.6466496 L69.8953318,31.6643661 L77.4636161,22.2040107 L80.8025651,18.4476931 L84.6980055,14.3018314 L87.2022173,12.3262866 L91.932395,12.3262866 L95.4104668,17.5016575 L93.8522906,22.8439759 L88.9829901,29.0210315 L84.9484267,34.2520515 L79.1609152,42.0429324 L75.5437204,48.2756372 L75.8776153,48.7764795 L76.7401772,48.6930058 L89.8177273,45.9105483 L96.8851693,44.6306179 L105.316015,43.18374 L109.127982,44.9645128 L109.545351,46.7731102 L108.042824,50.4737786 L99.0276615,52.6997446 L88.4543231,54.8144123 L72.7056138,58.5429053 L72.5108418,58.6820282 L72.7334384,58.9602739 L79.828705,59.6280637 L82.8615836,59.7950111 L90.2907451,59.7950111 L104.119559,60.8245204 L107.736753,63.2174338 L109.90707,66.1390142 L109.545351,68.3649802 L103.980436,71.2030868 L96.4678007,69.422314 L78.9383186,65.2486278 L72.9282104,63.7461008 L72.0934732,63.7461008 L72.0934732,64.2469431 L77.1018966,69.1440683 L86.2840063,77.4357915 L97.7755557,88.1204282 L98.3598717,90.7637628 L96.8851693,92.8506059 L95.3269931,92.6280093 L85.2266725,85.0319004 L81.331232,81.6094777 L72.5108418,74.1803163 L71.9265257,74.1803163 L71.9265257,74.9594044 L73.9577197,77.9366339 L84.6980055,94.0748872 L85.254497,99.0276615 L84.4754089,100.641487 L81.6929515,101.615347 L78.6322482,101.058855 L72.3438944,92.2384653 L65.8607684,82.3050921 L60.6297484,73.4012282 L59.9897832,73.7629477 L56.9012554,107.013314 L55.4543775,108.710614 L52.1154285,109.990544 L49.332971,107.875876 L47.8582686,104.453454 L49.332971,97.692082 L51.1137438,88.8716918 L52.5606217,81.8598989 L53.8683767,73.150807 L54.6474648,70.2570512 L54.5918157,70.0622792 L53.9518504,70.1457529 L47.3852508,79.1609152 L37.3962285,92.6558339 L29.4940492,101.114505 L27.6019782,101.865768 L24.3186783,100.168469 L24.6247487,97.1355905 L26.4611706,94.4366067 L37.3962285,80.5243193 L43.9906527,71.8987012 L48.2478126,66.9181023 L48.219988,66.1946633 L47.9695669,66.1946633 L18.9207108,85.059725 L13.7453399,85.7275148 L11.5193739,83.6406717 L11.7976197,80.218249 L12.8549535,79.105266 L21.59187,73.0951579 L21.5640454,73.1229824 Z" />
    </svg>
  )
}

export function GeminiEngineIcon({ size = 12 }: IconProps) {
  // Google Gemini 4-pointed star — official logomark path
  return (
    <svg width={size} height={size} viewBox="0 0 72.76 72.76" fill="currentColor" stroke="none">
      <path d="M36.38 72.76C33.53 53.97 18.79 39.23 0 36.38C18.79 33.53 33.53 18.79 36.38 0C39.23 18.79 53.97 33.53 72.76 36.38C53.97 39.23 39.23 53.97 36.38 72.76Z" />
    </svg>
  )
}

export function OpenCodeEngineIcon({ size = 12 }: IconProps) {
  // OpenCode two C-bracket brandmark — paths from opencode.ai/brand
  return (
    <svg width={size} height={size} viewBox="0 0 54 42" fill="currentColor" fillRule="evenodd" stroke="none">
      <path d="M18 12H6V30H18V12ZM24 36H0V6H24V36ZM36 30H48V12H36V30ZM54 36H36V42H30V6H54V36Z" />
    </svg>
  )
}

export function DefaultEngineIcon({ size = 12 }: IconProps) {
  // Generic sparkle
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" stroke="none">
      <path d="M12 2L13.5 10.5L22 12L13.5 13.5L12 22L10.5 13.5L2 12L10.5 10.5L12 2Z" />
    </svg>
  )
}

// ── Theme ─────────────────────────────────────────────────────────────────────

export function SunIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="2.5">
      <circle cx="12" cy="12" r="4" />
      <line x1="12" y1="2" x2="12" y2="4" />
      <line x1="12" y1="20" x2="12" y2="22" />
      <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
      <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
      <line x1="2" y1="12" x2="4" y2="12" />
      <line x1="20" y1="12" x2="22" y2="12" />
      <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
      <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
    </svg>
  )
}

export function MoonIcon({ size = 12 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="2.5">
      <path d="M21 12.79A9 9 0 1 1 11.21 3a7 7 0 0 0 9.79 9.79z" />
    </svg>
  )
}

// ── Window controls ───────────────────────────────────────────────────────────

export function MinimizeIcon({ size = 11 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="2.5">
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
  )
}

export function MaximizeIcon({ size = 10 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="2.5">
      <rect x="3" y="3" width="18" height="18" rx="1" />
    </svg>
  )
}

export function CloseIcon({ size = 11 }: IconProps) {
  return (
    <svg width={size} height={size} {...defaults} strokeWidth="2.5">
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </svg>
  )
}
