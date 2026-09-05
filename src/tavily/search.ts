import * as z from "zod/v4";

import { requestTavilyJson } from "./api.js";

const TAVILY_SEARCH_PATH = "/search";
const DEFAULT_MAX_RESULTS = 8;

export const TAVILY_SEARCH_DEPTHS = [
  "basic",
  "advanced",
] as const;

export const TAVILY_SEARCH_TOPICS = [
  "general",
  "news",
  "finance",
] as const;

export const TAVILY_TIME_RANGES = [
  "day",
  "week",
  "month",
  "year",
] as const;

export interface TavilySearchOptions {
  search_depth?: (typeof TAVILY_SEARCH_DEPTHS)[number];
  topic?: (typeof TAVILY_SEARCH_TOPICS)[number];
  time_range?: (typeof TAVILY_TIME_RANGES)[number];
  max_results?: number;
  include_domains?: string[];
  exclude_domains?: string[];
  include_answer?: "basic" | "advanced";
}

const searchResponseSchema = z
  .object({
    query: z.string(),
    answer: z.string().nullable().optional(),
    results: z.array(
      z
        .object({
          title: z.string(),
          url: z.string(),
          content: z.string(),
          score: z.number(),
          raw_content: z.string().nullable().optional(),
          favicon: z.string().nullable().optional(),
        })
        .passthrough(),
    ),
    response_time: z.union([z.string(), z.number()]),
    request_id: z.string(),
    usage: z
      .object({ credits: z.number() })
      .nullish(),
  })
  .passthrough();

export type TavilySearchResponse = z.infer<
  typeof searchResponseSchema
>;

export async function searchWithTavily(
  query: string,
  options: TavilySearchOptions = {},
): Promise<TavilySearchResponse> {
  const normalizedQuery = query.trim();

  if (!normalizedQuery) {
    throw new Error("The search query cannot be empty.");
  }

  const max_results =
    options.max_results ?? DEFAULT_MAX_RESULTS;

  if (
    !Number.isInteger(max_results) ||
    max_results < 1 ||
    max_results > 20
  ) {
    throw new Error(
      "max_results must be an integer between 1 and 20.",
    );
  }

  const requestBody = {
    query: normalizedQuery,
    search_depth: options.search_depth ?? "basic",
    topic: options.topic ?? "general",
    max_results,
    include_usage: true,
    include_raw_content: false,
    include_answer: options.include_answer ?? false,
    ...(options.time_range
      ? { time_range: options.time_range }
      : {}),
    ...(options.include_domains?.length
      ? { include_domains: options.include_domains }
      : {}),
    ...(options.exclude_domains?.length
      ? { exclude_domains: options.exclude_domains }
      : {}),
  };

  return requestTavilyJson(
    TAVILY_SEARCH_PATH,
    {
      method: "POST",
      body: requestBody,
    },
    searchResponseSchema,
    "Search",
  );
}
