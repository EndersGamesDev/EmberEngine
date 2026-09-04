import { z } from "zod";

export const CATEGORIES = [
  "Tiere & Natur",
  "Körper & Gesundheit",
  "Geschichte",
  "Geografie & Länder",
  "Essen & Trinken",
  "Technik & Internet",
  "Sport",
  "Weltraum & Wissenschaft",
  "Popkultur & Musik",
  "Alltag & Kurioses",
] as const;

export const categorySchema = z.enum(CATEGORIES);
export type Category = z.infer<typeof categorySchema>;

export const sourceSchema = z.object({
  title: z.string().min(1),
  // Zod 4: `z.string().url()` gibt es nicht mehr, der Format-Check heisst `z.url()`.
  url: z.url().startsWith("https://"),
});
export type Source = z.infer<typeof sourceSchema>;

export const questionSchema = z
  .object({
    id: z.string().regex(/^q\d{3}$/),
    category: categorySchema,
    question: z.string().min(40).max(200).endsWith("?"),
    answer: z.number().finite().positive(),
    unit: z.string(),
    answerFormat: z.enum(["integer", "decimal"]),
    explanation: z.string().min(120).max(500),
    sources: z.array(sourceSchema).min(1),
    difficulty: z.union([z.literal(1), z.literal(2), z.literal(3)]),
  })
  .strict();
export type Question = z.infer<typeof questionSchema>;
export const questionsSchema = z.array(questionSchema);
