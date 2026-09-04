import type { ReactNode } from "react";

import type { Question } from "@/data/schema";
import { CategoryBadge } from "./CategoryBadge";

export interface QuestionCardProps {
  question: Question;
  /** Eingabebereich; steckt in derselben Karte wie die Frage. */
  children?: ReactNode;
}

/** Fragekarte: Kategorie, Fragetext, Eingabe (Abschnitt 3.2). */
export function QuestionCard({ question, children }: QuestionCardProps) {
  return (
    <article className="card animate-card-in flex min-h-[24rem] flex-col p-4 sm:p-6">
      <CategoryBadge category={question.category} className="self-start" />
      <h2
        data-testid="question-text"
        className="font-display text-ink mt-3 text-xl leading-tight tracking-wide text-balance sm:text-2xl"
      >
        {question.question}
      </h2>
      <div className="mt-auto">{children}</div>
    </article>
  );
}
