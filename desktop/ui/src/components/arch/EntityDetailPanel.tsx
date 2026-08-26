import { Link, User, Users, Clock } from 'lucide-react'
import type { ArchEntityDto } from '../../types'

const CRITICALITY_CHIP: Record<string, string> = {
  critical: 'bg-red-100    text-red-700    dark:bg-red-900/40    dark:text-red-300',
  high:     'bg-orange-100 text-orange-700 dark:bg-orange-900/40 dark:text-orange-300',
  medium:   'bg-amber-100  text-amber-700  dark:bg-amber-900/40  dark:text-amber-300',
  low:      'bg-green-100  text-green-700  dark:bg-green-900/40  dark:text-green-300',
}

const LIFECYCLE_CHIP: Record<string, string> = {
  production:  'bg-rose-100  text-rose-700  dark:bg-rose-900/50  dark:text-rose-300',
  staging:     'bg-amber-100 text-amber-700 dark:bg-amber-900/50 dark:text-amber-300',
  development: 'bg-blue-100  text-blue-700  dark:bg-blue-900/50  dark:text-blue-300',
  deprecated:  'bg-zinc-200  text-zinc-500  dark:bg-zinc-800     dark:text-zinc-400',
}

interface Props {
  entity: ArchEntityDto
  allEntities: ArchEntityDto[]
  onClose: () => void
  onEdit: () => void
}

function Section({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-[9px] uppercase tracking-wider text-foreground/28 mb-1.5">{label}</div>
      {children}
    </div>
  )
}

export function EntityDetailPanel({ entity, allEntities }: Props) {
  const idToEntity = new Map(allEntities.map((e) => [e.id, e]))
  const crit = (entity.criticality ?? '').toLowerCase()
  const lc   = (entity.lifecycle ?? '').toLowerCase()

  return (
    <div className="flex flex-col h-full w-full overflow-hidden">
      {/* Header */}
      <div className="px-3 pt-3 pb-2.5 border-b border-border/40">
        <div className="text-[12px] font-semibold text-foreground leading-snug truncate">
          {entity.name}
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-3 py-3 space-y-4 text-[11px]">

        {/* Badges */}
        {(crit || lc) && (
          <div className="flex flex-wrap gap-1.5">
            {crit && (
              <span className={`px-2 py-0.5 rounded-full text-[10px] font-medium ${CRITICALITY_CHIP[crit] ?? 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400'}`}>
                {entity.criticality}
              </span>
            )}
            {lc && (
              <span className={`px-2 py-0.5 rounded-full text-[10px] font-medium ${LIFECYCLE_CHIP[lc] ?? 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400'}`}>
                {entity.lifecycle}
              </span>
            )}
          </div>
        )}

        {/* Kind */}
        <Section label="Kind">
          <span className="text-foreground/60">{entity.kind}</span>
        </Section>

        {/* Key */}
        <Section label="Key">
          <span className="font-mono text-[10px] text-foreground/60 bg-accent px-1.5 py-0.5 rounded select-all">
            {entity.id}
          </span>
        </Section>

        {/* Description */}
        {entity.description && (
          <Section label="Description">
            <p className="text-foreground/65 leading-relaxed">{entity.description}</p>
          </Section>
        )}

        {/* Ownership */}
        {(entity.owner || entity.team) && (
          <Section label="Ownership">
            <div className="space-y-1.5">
              {entity.owner && (
                <div className="flex items-center gap-2 text-foreground/65">
                  <User size={10} className="shrink-0 text-foreground/25" />
                  {entity.owner}
                </div>
              )}
              {entity.team && (
                <div className="flex items-center gap-2 text-foreground/65">
                  <Users size={10} className="shrink-0 text-foreground/25" />
                  {entity.team}
                </div>
              )}
            </div>
          </Section>
        )}

        {/* Tags */}
        {entity.tags.length > 0 && (
          <Section label="Tags">
            <div className="flex flex-wrap gap-1">
              {entity.tags.map((t) => (
                <span key={t} className="px-1.5 py-0.5 rounded bg-accent text-foreground/55 text-[10px]">
                  {t}
                </span>
              ))}
            </div>
          </Section>
        )}

        {/* Connections */}
        {entity.connections.length > 0 && (
          <Section label={`Connections · ${entity.connections.length}`}>
            <div className="space-y-1.5">
              {entity.connections.map((id) => {
                const target = idToEntity.get(id)
                return (
                  <div key={id} className="flex items-start gap-2">
                    <Link size={9} className="shrink-0 mt-0.5 text-foreground/22" />
                    {target ? (
                      <span className="text-foreground/65 leading-snug">
                        {target.name}
                        <span className="text-foreground/30 ml-1">· {target.kind}</span>
                      </span>
                    ) : (
                      <span className="text-amber-600 dark:text-amber-400 font-mono leading-snug">
                        {id}
                        <span className="opacity-55 font-sans ml-1">not found</span>
                      </span>
                    )}
                  </div>
                )
              })}
            </div>
          </Section>
        )}

        {/* Notes */}
        {entity.notes && (
          <Section label="Notes">
            <p className="text-foreground/55 leading-relaxed whitespace-pre-wrap">{entity.notes}</p>
          </Section>
        )}

        {/* Last updated */}
        {entity.last_updated && (
          <div className="flex items-center gap-1.5 text-foreground/28 text-[10px] pt-1">
            <Clock size={9} className="shrink-0" />
            {entity.last_updated}
          </div>
        )}
      </div>
    </div>
  )
}
