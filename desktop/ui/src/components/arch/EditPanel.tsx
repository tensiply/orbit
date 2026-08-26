import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Trash2 } from 'lucide-react'
import { useState } from 'react'
import { tauriService } from '../../services/tauri'
import type { ArchEntityDto, SaveEntityArgs } from '../../types'
import {
  Form, FormControl, FormField, FormItem, FormLabel, FormMessage,
} from '../ui/form'
import { Input } from '../ui/input'
import { Textarea } from '../ui/textarea'
import { Button } from '../ui/button'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '../ui/select'

const KIND_FOLDERS = [
  { label: 'Services',       folder: 'services'       },
  { label: 'Databases',      folder: 'databases'      },
  { label: 'Integrations',   folder: 'integrations'   },
  { label: 'Infrastructure', folder: 'infrastructure' },
  { label: 'APIs',           folder: 'apis'           },
  { label: 'Pipelines',      folder: 'pipelines'      },
  { label: 'Secrets',        folder: 'secrets'        },
  { label: 'IAM',            folder: 'iam'            },
  { label: 'Teams',          folder: 'teams'          },
]

const CRITICALITIES = ['critical', 'high', 'medium', 'low']
const LIFECYCLES    = ['production', 'development', 'planned', 'deprecated']

const schema = z.object({
  kindFolder:  z.string(),
  id:          z.string().min(1, 'Required'),
  name:        z.string().min(1, 'Required'),
  description: z.string(),
  criticality: z.string(),
  lifecycle:   z.string(),
  owner:       z.string(),
  team:        z.string(),
  tagsRaw:     z.string(),
  connections: z.array(z.string()),
  notes:       z.string(),
})

type FormValues = z.infer<typeof schema>

interface Props {
  workspace:   string
  tenant:      string
  entity:      ArchEntityDto | null
  allEntities: ArchEntityDto[]
  onSave:      (saved: ArchEntityDto, prev: ArchEntityDto | null) => void
  onDelete:    (entity: ArchEntityDto) => void
  onClose:     () => void
}

function today(): string {
  return new Date().toISOString().slice(0, 10)
}

function defaultValues(entity: ArchEntityDto | null): FormValues {
  return {
    kindFolder:  entity?.kind_folder      ?? 'services',
    id:          entity?.id               ?? '',
    name:        entity?.name             ?? '',
    description: entity?.description      ?? '',
    criticality: entity?.criticality      ?? '',
    lifecycle:   entity?.lifecycle        ?? '',
    owner:       entity?.owner            ?? '',
    team:        entity?.team             ?? '',
    tagsRaw:     entity?.tags.join(', ') ?? '',
    connections: entity?.connections      ?? [],
    notes:       entity?.notes            ?? '',
  }
}

export function EditPanel({ workspace, tenant, entity, allEntities, onSave, onDelete }: Props) {
  const isNew = entity === null
  const [confirmDel, setConfirmDel] = useState(false)
  const [saveError,  setSaveError]  = useState<string | null>(null)

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: defaultValues(entity),
  })

  useEffect(() => {
    form.reset(defaultValues(entity))
    setSaveError(null)
    setConfirmDel(false)
  }, [entity])

  // Auto-suggest id from name on create
  const watchedName = form.watch('name')
  const watchedId   = form.watch('id')
  useEffect(() => {
    if (isNew && watchedName && !watchedId) {
      form.setValue(
        'id',
        watchedName.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, ''),
        { shouldDirty: false },
      )
    }
  }, [watchedName, isNew])

  const onSubmit = form.handleSubmit(async (values) => {
    setSaveError(null)
    try {
      const args: SaveEntityArgs = {
        workspace, tenant,
        kind_folder:  values.kindFolder,
        id:           values.id.trim(),
        name:         values.name.trim(),
        description:  values.description.trim() || null,
        criticality:  values.criticality || null,
        lifecycle:    values.lifecycle   || null,
        owner:        values.owner.trim()  || null,
        team:         values.team.trim()   || null,
        tags:         values.tagsRaw.split(',').map((t) => t.trim()).filter(Boolean),
        connections:  values.connections,
        notes:        values.notes.trim() || null,
        last_updated: today(),
      }
      const saved = await tauriService.architectureSaveEntity(args)
      onSave(saved, entity)
    } catch (e) {
      setSaveError(String(e))
    }
  })

  const handleDelete = async () => {
    if (!entity) return
    if (!confirmDel) { setConfirmDel(true); return }
    try {
      await tauriService.architectureDeleteEntity(workspace, tenant, entity.kind_folder, entity.id)
      onDelete(entity)
    } catch (e) {
      setSaveError(String(e))
    }
  }

  const activeConnections = allEntities.filter((e) =>
    form.watch('connections').includes(e.id),
  )

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex h-[36px] items-center gap-2 px-3 py-2.5 border-b border-sidebar-border/40 shrink-0">
        <span className="font-semibold flex-1 truncate text-sm">
          {isNew ? 'New entity' : entity!.name}
        </span>
        {!isNew && (
          <button
            onClick={handleDelete}
            title={confirmDel ? 'Click again to confirm' : 'Delete entity'}
            className={`p-1 rounded transition-colors ${
              confirmDel
                ? 'text-destructive bg-destructive/10 hover:bg-destructive/20'
                : 'text-muted-foreground hover:text-destructive hover:bg-destructive/10'
            }`}
          >
          </button>
        )}
      </div>

      {/* Form */}
      <Form {...form}>
        <form onSubmit={onSubmit} className="flex flex-col flex-1 min-h-0">
          <div className="flex-1 overflow-y-auto px-3 py-3 space-y-3 min-h-0">

            {/* Kind (create only) */}
            {isNew && (
              <FormField
                control={form.control}
                name="kindFolder"
                render={({ field }) => (
                  <FormItem className="space-y-1">
                    <FieldLabel>Kind</FieldLabel>
                    <Select onValueChange={field.onChange} value={field.value}>
                      <FormControl>
                        <SelectTrigger className="h-8 text-xs">
                          <SelectValue />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        {KIND_FOLDERS.map((k) => (
                          <SelectItem key={k.folder} value={k.folder} className="text-xs">
                            {k.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </FormItem>
                )}
              />
            )}

            {/* ID */}
            <FormField
              control={form.control}
              name="id"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FieldLabel>ID</FieldLabel>
                  <FormControl>
                    <Input
                      {...field}
                      placeholder="e.g. orders-api"
                      readOnly={!isNew}
                      className={`h-8 text-xs font-mono ${!isNew ? 'opacity-50 cursor-not-allowed' : ''}`}
                    />
                  </FormControl>
                  <FormMessage className="text-[10px]" />
                </FormItem>
              )}
            />

            {/* Name */}
            <FormField
              control={form.control}
              name="name"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FieldLabel>Name</FieldLabel>
                  <FormControl>
                    <Input {...field} placeholder="Display name" className="h-8 text-xs" />
                  </FormControl>
                  <FormMessage className="text-[10px]" />
                </FormItem>
              )}
            />

            {/* Description */}
            <FormField
              control={form.control}
              name="description"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FieldLabel>Description</FieldLabel>
                  <FormControl>
                    <Textarea
                      {...field}
                      placeholder="Brief description…"
                      rows={4}
                      className="text-xs min-h-0 resize-none"
                    />
                  </FormControl>
                </FormItem>
              )}
            />

            {/* Criticality */}
            <FormField
              control={form.control}
              name="criticality"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FieldLabel>Criticality</FieldLabel>
                  <Select onValueChange={field.onChange} value={field.value}>
                    <FormControl>
                      <SelectTrigger className="h-8 text-xs">
                        <SelectValue placeholder="—" />
                      </SelectTrigger>
                    </FormControl>
                    <SelectContent>
                      {CRITICALITIES.map((c) => (
                        <SelectItem key={c} value={c} className="text-xs">{c}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FormItem>
              )}
            />

            {/* Lifecycle */}
            <FormField
              control={form.control}
              name="lifecycle"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FieldLabel>Lifecycle</FieldLabel>
                  <Select onValueChange={field.onChange} value={field.value}>
                    <FormControl>
                      <SelectTrigger className="h-8 text-xs">
                        <SelectValue placeholder="—" />
                      </SelectTrigger>
                    </FormControl>
                    <SelectContent>
                      {LIFECYCLES.map((l) => (
                        <SelectItem key={l} value={l} className="text-xs">{l}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FormItem>
              )}
            />

            {/* Owner */}
            <FormField
              control={form.control}
              name="owner"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FieldLabel>Owner</FieldLabel>
                  <FormControl>
                    <Input {...field} placeholder="owner" className="h-8 text-xs" />
                  </FormControl>
                </FormItem>
              )}
            />

            {/* Team */}
            <FormField
              control={form.control}
              name="team"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FieldLabel>Team</FieldLabel>
                  <FormControl>
                    <Input {...field} placeholder="team" className="h-8 text-xs" />
                  </FormControl>
                </FormItem>
              )}
            />

            {/* Tags */}
            <FormField
              control={form.control}
              name="tagsRaw"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <div className="flex items-baseline gap-1">
                    <FieldLabel>Tags</FieldLabel>
                    <span className="text-[9px] text-muted-foreground/60">comma-separated</span>
                  </div>
                  <FormControl>
                    <Input {...field} placeholder="tag1, tag2" className="h-8 text-xs" />
                  </FormControl>
                </FormItem>
              )}
            />

            {/* Connections — read-only, managed via diagram */}
            {activeConnections.length > 0 && (
              <div className="space-y-1">
                <div className="flex items-baseline gap-1">
                  <span className="text-[10px] text-muted-foreground uppercase tracking-wider">Connections</span>
                  <span className="text-[9px] text-muted-foreground/60">managed on diagram</span>
                </div>
                <div className="flex flex-wrap gap-1">
                  {activeConnections.map((e) => (
                    <span
                      key={e.id}
                      className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-accent text-accent-foreground font-mono text-[10px]"
                    >
                      {e.id}
                      <span className="text-muted-foreground/50 text-[9px]">{e.kind}</span>
                    </span>
                  ))}
                </div>
              </div>
            )}

            {/* Notes */}
            <FormField
              control={form.control}
              name="notes"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FieldLabel>Notes</FieldLabel>
                  <FormControl>
                    <Textarea
                      {...field}
                      placeholder="Internal notes…"
                      rows={4}
                      className="text-xs min-h-0 resize-none"
                    />
                  </FormControl>
                </FormItem>
              )}
            />

            {saveError && (
              <p className="text-destructive text-[10px] bg-destructive/10 border border-destructive/30 rounded px-2 py-1">
                {saveError}
              </p>
            )}
          </div>

          {/* Footer */}
          <div className="px-3 pb-3 pt-2 border-t border-sidebar-border/40 shrink-0 space-y-2">
            <Button
              type="submit"
              size="sm"
              className="w-full text-xs"
              disabled={form.formState.isSubmitting}
            >
              {form.formState.isSubmitting ? 'Saving…' : 'Save'}
            </Button>
            {!isNew && (
              <button
                type="button"
                onClick={handleDelete}
                className="w-full h-8 text-xs rounded-md border border-destructive/30 text-destructive bg-transparent hover:bg-destructive/30 transition-colors"
              >
                {confirmDel ? 'Confirm delete?' : 'Delete'}
              </button>
            )}
          </div>
        </form>
      </Form>
    </div>
  )
}

function FieldLabel({ children }: { children: React.ReactNode }) {
  return (
    <FormLabel className="text-[10px] text-muted-foreground uppercase tracking-wider">
      {children}
    </FormLabel>
  )
}
