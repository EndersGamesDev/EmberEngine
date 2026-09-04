/**
 * jsdom kennt `window.matchMedia` nicht. Der Ersatz hier beantwortet
 * `(prefers-reduced-motion: reduce)` je nach Testwunsch und alle anderen
 * Abfragen mit „trifft nicht zu“.
 */

const REDUCED_QUERY = "prefers-reduced-motion: reduce";

let reduced = false;

class FakeMediaQueryList extends EventTarget implements MediaQueryList {
  readonly media: string;
  readonly matches: boolean;
  onchange: ((this: MediaQueryList, event: MediaQueryListEvent) => unknown) | null = null;

  constructor(media: string, matches: boolean) {
    super();
    this.media = media;
    this.matches = matches;
  }

  /** Veraltete Zwillinge von add/removeEventListener; hier ohne Wirkung. */
  addListener(): void {}
  removeListener(): void {}
}

/** Hängt den Ersatz an `window`. Wird vom globalen Test-Setup aufgerufen. */
export function installMatchMedia(): void {
  window.matchMedia = (query: string): MediaQueryList =>
    new FakeMediaQueryList(query, reduced && query.includes(REDUCED_QUERY));
}

/** Schaltet `prefers-reduced-motion` für den laufenden Test ein oder aus. */
export function setReducedMotion(value: boolean): void {
  reduced = value;
}

export function resetReducedMotion(): void {
  reduced = false;
}
