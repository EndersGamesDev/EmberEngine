"use client";

import { motion } from "motion/react";
import { useEffect, useMemo, useState } from "react";

import { useReducedMotionSafe } from "./useReducedMotionSafe";

const PARTICLE_COUNT = 40;
const LIFETIME_MS = 2500;

const PARTICLE_COLORS = ["#F2542D", "#E38A00", "#57D175", "#9BD1F5", "#F9A8DA", "#FFF8E7"];

interface Particle {
  id: number;
  left: number;
  size: number;
  color: string;
  delay: number;
  duration: number;
  drift: number;
  rotation: number;
  round: boolean;
}

function createParticles(): Particle[] {
  return Array.from({ length: PARTICLE_COUNT }, (_unused, index) => ({
    id: index,
    left: Math.random() * 100,
    size: 8 + Math.random() * 10,
    color: PARTICLE_COLORS[index % PARTICLE_COLORS.length] ?? "#F2542D",
    delay: Math.random() * 0.5,
    duration: 1.4 + Math.random() * 0.8,
    drift: (Math.random() - 0.5) * 120,
    rotation: Math.random() * 720 - 360,
    round: index % 3 === 0,
  }));
}

/**
 * Kurzer Konfetti-Regen bei „Volltreffer!“ (Abschnitt 3.3).
 *
 * Ohne Zusatzbibliothek: 40 bunte Teilchen, die mit `motion` von oben über den
 * ganzen Viewport fallen (Endposition in `vh`, damit sie unabhängig von der
 * Fenstergröße unten ankommen). Nach 2,5 Sekunden ist der Spuk vorbei. Bei
 * `prefers-reduced-motion` entsteht gar kein Teilchen.
 *
 * Die Zufallswerte entstehen erst beim ersten Render – der findet immer im
 * Browser statt, weil die Auflösung bis zur Hydration gar nicht gerendert
 * wird. Ein Hydration-Unterschied ist damit ausgeschlossen.
 */
export function Confetti() {
  const reducedMotion = useReducedMotionSafe();
  const [expired, setExpired] = useState(false);
  const particles = useMemo(() => (reducedMotion ? [] : createParticles()), [reducedMotion]);

  useEffect(() => {
    if (reducedMotion) return undefined;
    const timer = window.setTimeout(() => {
      setExpired(true);
    }, LIFETIME_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [reducedMotion]);

  if (reducedMotion || expired) return null;

  return (
    <div aria-hidden="true" className="pointer-events-none fixed inset-0 z-50 overflow-hidden">
      {particles.map((particle) => (
        <motion.span
          key={particle.id}
          className="absolute top-0 block"
          style={{
            left: `${particle.left}%`,
            width: particle.size,
            height: particle.round ? particle.size : particle.size * 1.6,
            backgroundColor: particle.color,
            borderRadius: particle.round ? "9999px" : "2px",
            border: "2px solid #1B1B1B",
          }}
          initial={{ y: "-15vh", x: 0, rotate: 0, opacity: 1 }}
          animate={{
            y: "110vh",
            x: particle.drift,
            rotate: particle.rotation,
            opacity: [1, 1, 0],
          }}
          transition={{ duration: particle.duration, delay: particle.delay, ease: "easeIn" }}
        />
      ))}
    </div>
  );
}
