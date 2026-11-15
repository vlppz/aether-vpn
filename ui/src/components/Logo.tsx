/** Aether mark: a solid body with one tilted orbit. Monochrome (currentColor). */
export function Logo({ size = 18, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="none" className={className} aria-hidden>
      <ellipse cx="16" cy="16" rx="13" ry="5.2" transform="rotate(-24 16 16)" stroke="currentColor" strokeWidth="1.4" />
      <circle cx="16" cy="16" r="4.6" fill="currentColor" />
    </svg>
  );
}
