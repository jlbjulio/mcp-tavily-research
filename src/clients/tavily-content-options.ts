// List the extraction depths shared by Extract and Crawl.
export const TAVILY_EXTRACT_DEPTHS = [
  "basic",
  "advanced",
] as const;

// List the content formats shared by Extract and Crawl.
export const TAVILY_CONTENT_FORMATS = [
  "markdown",
  "text",
] as const;

// Derive TypeScript types directly from the supported values.
export type TavilyExtractDepth =
  (typeof TAVILY_EXTRACT_DEPTHS)[number];

export type TavilyContentFormat =
  (typeof TAVILY_CONTENT_FORMATS)[number];

// Represent an image returned by Tavily in one consistent shape.
export interface TavilyImage {
  url: string;
  description: string | null;
}

/**
 * Convert Tavily's string or object image representation into one shape.
 */
export function normalizeTavilyImage(
  image:
    | string
    | {
        url: string;
        description?: string | null | undefined;
      },
): TavilyImage {
  if (typeof image === "string") {
    return {
      url: image,
      description: null,
    };
  }

  return {
    url: image.url,
    description: image.description ?? null,
  };
}
