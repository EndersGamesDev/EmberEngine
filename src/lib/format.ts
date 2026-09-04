/**
 * Zahlen lesen und schreiben – in deutscher Schreibweise.
 *
 * Reine Logik: kein React, keine Browser-APIs (`Intl` ist Teil der Sprache und
 * steht auch auf dem Server zur Verfügung).
 */

/** Nur Ziffern, Punkt, Komma und Leerraum sind als Eingabe zugelassen. */
const ALLOWED_CHARACTERS = /^[\d.,\s]+$/;

/** Eine nicht leere Folge aus Ziffern. */
const DIGITS_ONLY = /^\d+$/;

/** Korrekt gruppierte Tausenderpunkte: „1.250“, „1.250.000“, aber nicht „1.25.000“. */
const THOUSAND_GROUPED = /^\d{1,3}(?:\.\d{3})+$/;

/** Standardwert für `maximumFractionDigits`, entspricht dem Intl-Standard. */
const DEFAULT_MAX_FRACTION_DIGITS = 3;

export interface FormatOptions {
  maximumFractionDigits?: number;
}

function countOccurrences(value: string, needle: string): number {
  return value.split(needle).length - 1;
}

/**
 * Wandelt einen Ganzzahl-Teil („1.250“, „1250“) in reine Ziffern um.
 * Gibt `null` zurück, wenn die Tausendergruppierung nicht stimmt.
 */
function toPlainDigits(integerPart: string): string | null {
  if (DIGITS_ONLY.test(integerPart)) return integerPart;
  if (THOUSAND_GROUPED.test(integerPart)) return integerPart.replaceAll(".", "");
  return null;
}

/**
 * Liest eine deutsch geschriebene Zahl aus einer Nutzereingabe.
 *
 * Erlaubt sind Tausenderpunkte („1.250.000“), Dezimalkomma („12,5“) und – weil
 * Handytastaturen oft nur einen Punkt anbieten – auch der Dezimalpunkt
 * („12.5“). Ein einzelner Punkt vor genau drei Ziffern gilt als Tausenderpunkt
 * („1.250“ = 1250), sonst als Dezimalpunkt („1.2345“ = 1,2345). Vorzeichen und
 * Exponentialschreibweise sind nicht zugelassen.
 *
 * @returns die Zahl oder `null`, wenn die Eingabe leer oder ungültig ist.
 */
export function parseGermanNumber(input: string): number | null {
  const trimmed = input.trim();
  if (trimmed === "") return null;
  if (!ALLOWED_CHARACTERS.test(trimmed)) return null;

  // Leerzeichen innerhalb der Zahl („1 250 000“) sind als Gruppentrenner üblich.
  const compact = trimmed.replace(/\s/g, "");
  if (compact === "") return null;

  const commaCount = countOccurrences(compact, ",");
  if (commaCount > 1) return null;

  let normalized: string;

  if (commaCount === 1) {
    // Mit Komma ist das Komma immer das Dezimaltrennzeichen; Punkte davor
    // müssen eine gültige Tausendergruppierung bilden.
    const commaIndex = compact.indexOf(",");
    const integerPart = compact.slice(0, commaIndex);
    const fractionPart = compact.slice(commaIndex + 1);
    if (!DIGITS_ONLY.test(fractionPart)) return null;
    const digits = toPlainDigits(integerPart);
    if (digits === null) return null;
    normalized = `${digits}.${fractionPart}`;
  } else {
    const dotCount = countOccurrences(compact, ".");
    if (dotCount === 0) {
      if (!DIGITS_ONLY.test(compact)) return null;
      normalized = compact;
    } else if (dotCount === 1) {
      const dotIndex = compact.indexOf(".");
      const before = compact.slice(0, dotIndex);
      const after = compact.slice(dotIndex + 1);
      if (!DIGITS_ONLY.test(before) || !DIGITS_ONLY.test(after)) return null;
      normalized = THOUSAND_GROUPED.test(compact) ? before + after : `${before}.${after}`;
    } else {
      if (!THOUSAND_GROUPED.test(compact)) return null;
      normalized = compact.replaceAll(".", "");
    }
  }

  const value = Number(normalized);
  return Number.isFinite(value) ? value : null;
}

/**
 * Formatiert eine Zahl in deutscher Schreibweise: Punkt als Tausendertrenner,
 * Komma als Dezimaltrenner.
 */
export function formatNumber(value: number, options?: FormatOptions): string {
  const maximumFractionDigits = options?.maximumFractionDigits ?? DEFAULT_MAX_FRACTION_DIGITS;
  const formatter = new Intl.NumberFormat("de-DE", { maximumFractionDigits });

  // Sehr große Ganzzahlen: `Intl` rundet einen Double intern auf rund 16
  // signifikante Stellen und macht aus 43.252.003.274.489.856.000 (den
  // Stellungen eines Zauberwürfels) „43.252.003.274.489.860.000“. Als BigInt
  // formatiert Intl exakt den Wert des Doubles – und das ist genau die Zahl,
  // die in der Frage steht.
  if (Number.isInteger(value) && Math.abs(value) > Number.MAX_SAFE_INTEGER) {
    return formatter.format(BigInt(value));
  }

  return formatter.format(value);
}

/**
 * Formatiert eine Zahl mit Einheit, z. B. „1.250.000 km“ oder „71 %“.
 * Bei leerer Einheit bleibt nur die Zahl übrig.
 */
export function formatWithUnit(value: number, unit: string, options?: FormatOptions): string {
  const formatted = formatNumber(value, options);
  const trimmedUnit = unit.trim();
  return trimmedUnit === "" ? formatted : `${formatted} ${trimmedUnit}`;
}
