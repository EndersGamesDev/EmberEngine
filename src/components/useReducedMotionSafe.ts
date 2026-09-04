"use client";

import { useSyncExternalStore } from "react";

const QUERY = "(prefers-reduced-motion: reduce)";

function getMediaQueryList(): MediaQueryList | null {
  if (typeof window === "undefined") return null;
  if (typeof window.matchMedia !== "function") return null;
  return window.matchMedia(QUERY);
}

function subscribe(onChange: () => void): () => void {
  const list = getMediaQueryList();
  if (list === null) return () => undefined;
  list.addEventListener("change", onChange);
  return () => {
    list.removeEventListener("change", onChange);
  };
}

function getSnapshot(): boolean {
  return getMediaQueryList()?.matches ?? false;
}

/** Auf dem Server ist die Einstellung unbekannt; „nicht reduziert“ ist der neutrale Wert. */
function getServerSnapshot(): boolean {
  return false;
}

/**
 * Ob der Nutzer `prefers-reduced-motion: reduce` gesetzt hat.
 *
 * `useSyncExternalStore` statt `useState` + `useEffect`: So steht der echte
 * Wert schon beim ersten Client-Render fest, und der Count-up beginnt gar
 * nicht erst bei null, bevor ein Effekt ihn abschaltet.
 */
export function useReducedMotionSafe(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
