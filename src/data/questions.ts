import { block1 } from "./blocks/block-1";
import { block2 } from "./blocks/block-2";
import { block3 } from "./blocks/block-3";
import { block4 } from "./blocks/block-4";
import { block5 } from "./blocks/block-5";
import { type Question, questionsSchema } from "./schema";

/**
 * Alle Fragen; die Validierung beim Import ist zugleich die Validierung beim Build.
 * Fehlerhafte Daten brechen den Build ab, statt erst zur Laufzeit aufzufallen.
 */
export const questions: Question[] = questionsSchema.parse([
  ...block1,
  ...block2,
  ...block3,
  ...block4,
  ...block5,
]);
