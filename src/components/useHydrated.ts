"use client";

import { useSyncExternalStore } from "react";

/** Der Wert ändert sich nie; ein Abonnement ist nur formal nötig. */
function subscribe(): () => void {
  return () => undefined;
}

function getClientSnapshot(): boolean {
  return true;
}

function getServerSnapshot(): boolean {
  return false;
}

/**
 * `false` auf dem Server und im Hydration-Durchlauf, danach `true`.
 *
 * Bewusst über `useSyncExternalStore` statt über `useState` + `useEffect`:
 * React kennt den Unterschied zwischen Server- und Client-Schnappschuss und
 * plant die zweite Darstellung selbst ein – ohne `setState` in einem Effekt,
 * das die Regel `react-hooks/set-state-in-effect` (zu Recht) verbietet.
 * Alles, was aus `localStorage` oder `sessionStorage` kommt, wird erst
 * gerendert, wenn dieser Wert `true` ist.
 */
export function useHydrated(): boolean {
  return useSyncExternalStore(subscribe, getClientSnapshot, getServerSnapshot);
}
