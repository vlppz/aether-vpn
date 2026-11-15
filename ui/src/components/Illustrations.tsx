/** Hairline star-chart drawings for empty states (monochrome, currentColor). */

export function EmptyServersArt() {
  return (
    <svg width="132" height="96" viewBox="0 0 132 96" fill="none" stroke="currentColor" aria-hidden>
      <ellipse cx="66" cy="48" rx="58" ry="20" transform="rotate(-14 66 48)" strokeWidth="0.75" />
      <ellipse cx="66" cy="48" rx="36" ry="36" strokeWidth="0.75" strokeDasharray="1.5 3" />
      <circle cx="66" cy="48" r="6" strokeWidth="1" />
      <circle cx="12" cy="60" r="2" fill="currentColor" stroke="none" opacity=".5" />
      <circle cx="118" cy="34" r="2" fill="currentColor" stroke="none" opacity=".5" />
      <path d="M100 14v8M96 18h8" strokeWidth="0.75" />
    </svg>
  );
}

export function EmptySubsArt() {
  return (
    <svg width="132" height="96" viewBox="0 0 132 96" fill="none" stroke="currentColor" aria-hidden>
      <circle cx="66" cy="48" r="40" strokeWidth="0.75" />
      <circle cx="66" cy="48" r="26" strokeWidth="0.75" strokeDasharray="1.5 3" />
      <circle cx="66" cy="48" r="4" fill="currentColor" stroke="none" />
      <path d="M66 4v8M66 84v8M22 48h-8M118 48h-8" strokeWidth="0.75" />
      <circle cx="94" cy="20" r="2.5" strokeWidth="1" />
    </svg>
  );
}
