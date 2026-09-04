import type { Metadata } from "next";

import { ResultScreen } from "@/components/ResultScreen";

export const metadata: Metadata = {
  title: "Ergebnis – Verschätz dich",
};

export default function ErgebnisPage() {
  return <ResultScreen />;
}
