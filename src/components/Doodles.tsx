import type { ReactNode } from "react";

const INK = "#1B1B1B";

interface DoodleProps {
  /** Tailwind-Klassen für Position, Größe und Sichtbarkeit je Breakpoint. */
  className: string;
  children: ReactNode;
  title?: string;
}

function Doodle({ className, children }: DoodleProps) {
  return <span className={`absolute block ${className}`}>{children}</span>;
}

function Star({ fill }: { fill: string }) {
  return (
    <svg viewBox="0 0 48 48" className="h-full w-full" role="presentation">
      <path
        d="M24 3l5.9 13.2L44 18.1 33.6 28.3 36.3 43 24 36.1 11.7 43l2.7-14.7L4 18.1l14.1-1.9z"
        fill={fill}
        stroke={INK}
        strokeWidth="3"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function Bolt({ fill }: { fill: string }) {
  return (
    <svg viewBox="0 0 32 48" className="h-full w-full" role="presentation">
      <path
        d="M20 2L5 27h10L12 46 28 20H17z"
        fill={fill}
        stroke={INK}
        strokeWidth="3"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function QuestionMark({ fill }: { fill: string }) {
  return (
    <svg viewBox="0 0 40 56" className="h-full w-full" role="presentation">
      <path
        d="M9 17c0-7 5-12 11-12s11 4.5 11 11c0 8-9 9-9 16"
        fill="none"
        stroke={INK}
        strokeWidth="5"
        strokeLinecap="round"
      />
      <circle cx="22" cy="47" r="4.5" fill={fill} stroke={INK} strokeWidth="4" />
    </svg>
  );
}

function Wave() {
  return (
    <svg viewBox="0 0 96 24" className="h-full w-full" role="presentation">
      <path
        d="M2 16c6-14 14-14 20 0s14 14 20 0 14-14 20 0 14 14 20 0"
        fill="none"
        stroke={INK}
        strokeWidth="4"
        strokeLinecap="round"
      />
    </svg>
  );
}

function Spiral() {
  return (
    <svg viewBox="0 0 48 48" className="h-full w-full" role="presentation">
      <path
        d="M24 24c0-4 4-6 7-4s3 8-2 10-12-1-13-8 6-15 14-15 17 8 17 18"
        fill="none"
        stroke={INK}
        strokeWidth="4"
        strokeLinecap="round"
      />
    </svg>
  );
}

/**
 * Handgezeichnet wirkende Kritzeleien im Hintergrund (Abschnitt 6.3).
 *
 * Der Container ist `fixed` und `overflow-hidden`: So kann keine Kritzelei
 * jemals horizontales Scrollen auslösen. Auf schmalen Geräten bleiben nur
 * wenige, kleine Elemente übrig, damit die Fläche hinter dem Inhalt ruhig
 * bleibt.
 */
export function Doodles() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none fixed inset-0 -z-10 overflow-hidden select-none"
    >
      <Doodle className="top-[5%] left-[2%] h-8 w-8 rotate-[-12deg] opacity-45 sm:h-14 sm:w-14 sm:opacity-70">
        <Star fill="#FFF8E7" />
      </Doodle>
      <Doodle className="top-[12%] right-[6%] h-12 w-8 rotate-[10deg] opacity-70 sm:h-16 sm:w-11">
        <Bolt fill="#F2542D" />
      </Doodle>
      <Doodle className="hidden sm:top-[30%] sm:left-[7%] sm:block sm:h-16 sm:w-11 sm:rotate-[8deg] sm:opacity-60">
        <QuestionMark fill="#FFF8E7" />
      </Doodle>
      <Doodle className="bottom-[8%] left-[8%] h-6 w-24 opacity-60 sm:h-8 sm:w-32">
        <Wave />
      </Doodle>
      <Doodle className="hidden sm:right-[8%] sm:bottom-[16%] sm:block sm:h-14 sm:w-14 sm:rotate-[-8deg] sm:opacity-60">
        <Spiral />
      </Doodle>
      <Doodle className="right-[5%] bottom-[6%] h-8 w-8 rotate-[16deg] opacity-70 sm:h-12 sm:w-12">
        <Star fill="#E38A00" />
      </Doodle>
      <Doodle className="hidden lg:top-[62%] lg:left-[3%] lg:block lg:h-12 lg:w-8 lg:rotate-[-14deg] lg:opacity-60">
        <Bolt fill="#FFF8E7" />
      </Doodle>
      <Doodle className="hidden lg:top-[8%] lg:right-[22%] lg:block lg:h-14 lg:w-10 lg:rotate-[12deg] lg:opacity-50">
        <QuestionMark fill="#E38A00" />
      </Doodle>
      <Doodle className="hidden lg:top-[46%] lg:right-[4%] lg:block lg:h-8 lg:w-32 lg:opacity-50">
        <Wave />
      </Doodle>
    </div>
  );
}
