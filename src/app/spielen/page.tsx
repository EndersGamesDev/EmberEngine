import type { Metadata } from "next";

import { PlayScreen } from "@/components/PlayScreen";

export const metadata: Metadata = {
  title: "Frage – Verschätz dich",
};

export default function SpielenPage() {
  return <PlayScreen />;
}
