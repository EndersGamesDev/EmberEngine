import { type Category, type Question, questionSchema } from "@/data/schema";

/**
 * Test-Fixtures. Jede Frage laeuft durch `questionSchema.parse`, damit die
 * Fixtures nicht unbemerkt vom echten Datenvertrag abweichen.
 */

const BASE_EXPLANATION =
  "Diese Erklärung dient ausschließlich dem Test und ist genau deshalb so ausführlich formuliert, " +
  "dass sie die vom Schema geforderte Mindestlänge von 120 Zeichen sicher erreicht.";

const BASE_QUESTION: Question = {
  id: "q001",
  category: "Tiere & Natur",
  question: "Wie viele Beispielwerte stecken in dieser Testfrage?",
  answer: 100,
  unit: "Stück",
  answerFormat: "integer",
  explanation: BASE_EXPLANATION,
  sources: [{ title: "Wikipedia: Beispiel", url: "https://de.wikipedia.org/wiki/Beispiel" }],
  difficulty: 1,
};

/** Baut eine schema-gültige Frage; alle Felder sind überschreibbar. */
export function makeQuestion(overrides: Partial<Question> = {}): Question {
  return questionSchema.parse({ ...BASE_QUESTION, ...overrides });
}

/**
 * Baut `count` Fragen mit den IDs `q001`, `q002` … und verteilt sie reihum auf
 * die übergebenen Kategorien.
 */
export function makeQuestions(
  count: number,
  categories: readonly Category[] = ["Tiere & Natur"],
): Question[] {
  return Array.from({ length: count }, (_unused, position) => {
    const number = position + 1;
    const category = categories[position % categories.length] ?? BASE_QUESTION.category;
    return makeQuestion({
      id: `q${String(number).padStart(3, "0")}`,
      category,
      answer: number * 10,
      question: `Wie viele Beispielwerte stecken in der Testfrage Nummer ${number}?`,
    });
  });
}

/** Zieht die IDs aus einer Fragenliste – in Tests dauernd gebraucht. */
export function idsOf(questions: readonly Question[]): string[] {
  return questions.map((question) => question.id);
}
