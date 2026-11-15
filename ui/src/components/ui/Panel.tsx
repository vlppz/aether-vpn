import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../lib/cn";

/** The single surface level: flat, hairline border, small radius. */
export const Panel = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(function Panel({ className, ...rest }, ref) {
  return <div ref={ref} className={cn("panel", className)} {...rest} />;
});

interface SectionProps extends Omit<HTMLAttributes<HTMLDivElement>, "title"> {
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}

/** Panel with a letter-spaced caption header. */
export function Section({ title, description, actions, className, children, ...rest }: SectionProps) {
  return (
    <Panel className={cn("px-5 py-4", className)} {...rest}>
      {(title || actions) && (
        <div className="mb-3 flex items-start gap-3">
          <div className="min-w-0 flex-1">
            {title && <h2 className="label">{title}</h2>}
            {description && <p className="mt-1 text-[12.5px] text-ink-2">{description}</p>}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
        </div>
      )}
      {children}
    </Panel>
  );
}
