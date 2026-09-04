import type { Metadata } from "next";
import { Luckiest_Guy, Nunito } from "next/font/google";
import type { ReactNode } from "react";

import { Doodles } from "@/components/Doodles";
import { GameProvider } from "@/components/GameProvider";
import "./globals.css";

// Display-Schrift fuer Titel, Zahlen und Labels (Abschnitt 6.2).
const luckiestGuy = Luckiest_Guy({
  weight: "400",
  subsets: ["latin"],
  display: "swap",
  variable: "--font-display",
});

// Fliesstext-Schrift.
const nunito = Nunito({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-body",
});

export const metadata: Metadata = {
  title: "Verschätz dich",
  description: "Daneben ist auch drin. Ein Schätzspiel mit schrägen Fragen und echten Zahlen.",
};

// Bewusst eigener Props-Typ statt des globalen `LayoutProps<"/">`:
// dessen Deklaration liegt unter `.next/types`, und `.next` ist in der
// tsconfig ausgeschlossen (siehe DECISIONS.md).
export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="de" className={`${luckiestGuy.variable} ${nunito.variable}`}>
      <body className="bg-bg text-ink font-body min-h-dvh overflow-x-hidden antialiased">
        <Doodles />
        <GameProvider>{children}</GameProvider>
      </body>
    </html>
  );
}
