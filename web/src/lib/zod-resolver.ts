import type { FieldValues, Resolver } from "react-hook-form";
import type { ZodSchema } from "zod";

export function zodResolver<T extends FieldValues>(schema: ZodSchema<T>): Resolver<T> {
  // @ts-expect-error -- react-hook-form Resolver<T> types are overly narrow; runtime is correct.
  return async (values: T) => {
    const result = schema.safeParse(values);

    if (result.success) {
      return { values: result.data, errors: {} };
    }

    const errors: Record<string, { type: string; message: string }> = {};
    for (const issue of result.error.issues) {
      const key = issue.path.join(".");
      if (key && !(key in errors)) {
        errors[key] = { type: issue.code, message: issue.message };
      }
    }

    return { values: {}, errors };
  };
}
