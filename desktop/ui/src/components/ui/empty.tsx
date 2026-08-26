import * as React from 'react'
import { cn } from '@/lib/utils'

function Empty({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn('flex flex-col items-center justify-center gap-4 p-8 text-center', className)}
      {...props}
    />
  )
}

function EmptyHeader({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cn('flex flex-col items-center gap-2', className)} {...props} />
  )
}

interface EmptyMediaProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'icon' | 'image'
}

function EmptyMedia({ className, variant = 'icon', ...props }: EmptyMediaProps) {
  return (
    <div
      className={cn(
        'flex items-center justify-center',
        variant === 'icon' &&
          'size-12 rounded-full bg-muted text-muted-foreground [&_svg]:size-6',
        className,
      )}
      {...props}
    />
  )
}

function EmptyTitle({ className, ...props }: React.HTMLAttributes<HTMLHeadingElement>) {
  return (
    <h3 className={cn('text-base font-semibold', className)} {...props} />
  )
}

function EmptyDescription({ className, ...props }: React.HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p className={cn('text-sm text-muted-foreground', className)} {...props} />
  )
}

function EmptyContent({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cn('flex flex-col items-center gap-2', className)} {...props} />
  )
}

export { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle }
