type ClassValue = string | false | null | undefined | 0;

/** Tiny className joiner (no conflict resolution — later classes simply append). */
export function cn(...values: ClassValue[]): string {
  return values.filter(Boolean).join(" ");
}
