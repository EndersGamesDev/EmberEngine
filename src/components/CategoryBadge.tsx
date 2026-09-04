import type { Category } from "@/data/schema";

/**
 * Je Kategorie eine eigene Füllfarbe. Alle Töne sind bewusst hell gehalten:
 * Beschriftung und Kontur sind `--color-ink` (#1B1B1B), damit der Kontrast
 * überall deutlich über den 4,5:1 der WCAG-Stufe AA liegt.
 */
export const CATEGORY_COLORS: Record<Category, string> = {
  "Tiere & Natur": "#9FE39A",
  "Körper & Gesundheit": "#FFB3C1",
  Geschichte: "#EDC96B",
  "Geografie & Länder": "#9BD1F5",
  "Essen & Trinken": "#FFC08A",
  "Technik & Internet": "#C6B4F2",
  Sport: "#8FE0D8",
  "Weltraum & Wissenschaft": "#B8C6FF",
  "Popkultur & Musik": "#F9A8DA",
  "Alltag & Kurioses": "#F4E37A",
};

export interface CategoryBadgeProps {
  category: Category;
  className?: string;
}

export function CategoryBadge({ category, className }: CategoryBadgeProps) {
  return (
    <span
      data-testid="category-badge"
      className={`font-display border-ink text-ink inline-block -rotate-1 rounded-lg border-[3px] px-3 py-1 text-sm tracking-wide shadow-[3px_3px_0_0_var(--color-ink)] ${className ?? ""}`}
      style={{ backgroundColor: CATEGORY_COLORS[category] }}
    >
      {category}
    </span>
  );
}
