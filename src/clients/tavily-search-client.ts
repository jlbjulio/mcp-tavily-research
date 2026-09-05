// Import Zod to validate data received from the Tavily API.
import * as z from "zod/v4";

// Import the shared authenticated HTTP transport.
import { requestTavilyJson } from "./tavily-http-client.js";

// Store the endpoint path used by the shared HTTP client.
const TAVILY_SEARCH_PATH = "/search";

// Return a useful number of sources without producing excessive output.
const DEFAULT_MAX_RESULTS = 8;

// List the search depths supported by this MCP server.
export const TAVILY_SEARCH_DEPTHS = [
  "basic",
  "advanced",
] as const;

// List the search topics supported by Tavily.
export const TAVILY_SEARCH_TOPICS = [
  "general",
  "news",
  "finance",
] as const;

// List the time filters exposed by this client.
export const TAVILY_TIME_RANGES = [
  "day",
  "week",
  "month",
  "year",
] as const;

// Derive TypeScript types from the constant arrays.
export type TavilySearchDepth =
  (typeof TAVILY_SEARCH_DEPTHS)[number];

export type TavilySearchTopic =
  (typeof TAVILY_SEARCH_TOPICS)[number];

export type TavilyTimeRange =
  (typeof TAVILY_TIME_RANGES)[number];

// Describe the optional settings accepted by a search.
export interface TavilySearchOptions {
  searchDepth?: TavilySearchDepth;
  topic?: TavilySearchTopic;
  timeRange?: TavilyTimeRange;
  maxResults?: number;
  includeDomains?: string[];
  excludeDomains?: string[];
  includeAnswer?: "basic" | "advanced";
}

// Validate each result returned by the external API.
const searchResultSchema = z
  .object({
    title: z.string(),
    url: z.string(),
    content: z.string(),
    score: z.number(),
    raw_content: z.string().nullable().optional(),
    favicon: z.string().nullable().optional(),
  })
  .passthrough();

// Validate the portion of the Tavily response used by this project.
const searchResponseSchema = z
  .object({
    query: z.string(),
    answer: z.string().nullable().optional(),
    results: z.array(searchResultSchema),
    response_time: z.union([z.string(), z.number()]),
    request_id: z.string(),
    usage: z
      .object({
        credits: z.number(),
      })
      .nullish(),
  })
  .passthrough();

// Describe one normalized search result.
export interface TavilySearchResult {
  title: string;
  url: string;
  content: string;
  score: number;
  rawContent: string | null;
  favicon: string | null;
}

// Describe the normalized response returned to the MCP layer.
export interface TavilySearchResponse {
  query: string;
  answer: string | null;
  results: TavilySearchResult[];
  responseTime: string;
  requestId: string;
  creditsUsed: number | null;
}

/**
 * Search the web through the Tavily Search API.
 */
export async function searchWithTavily(
  query: string,
  options: TavilySearchOptions = {},
): Promise<TavilySearchResponse> {
  // Remove accidental whitespace from the query.
  const normalizedQuery = query.trim();

  // Reject empty searches before consuming an API credit.
  if (!normalizedQuery) {
    throw new Error("The search query cannot be empty.");
  }

  // Apply a default and validate Tavily's result-count limits.
  const maxResults =
    options.maxResults ?? DEFAULT_MAX_RESULTS;

  if (
    !Number.isInteger(maxResults) ||
    maxResults < 1 ||
    maxResults > 20
  ) {
    throw new Error(
      "maxResults must be an integer between 1 and 20.",
    );
  }

  // Build the JSON body expected by Tavily.
  const requestBody = {
    query: normalizedQuery,
    search_depth: options.searchDepth ?? "basic",
    topic: options.topic ?? "general",
    max_results: maxResults,

    // Ask Tavily to report the credits consumed by this request.
    include_usage: true,

    // Return only snippets for now to keep MCP output manageable.
    include_raw_content: false,

    // Tavily can optionally generate a quick or advanced answer.
    include_answer: options.includeAnswer ?? false,

    // Add time filtering only when one was requested.
    ...(options.timeRange
      ? { time_range: options.timeRange }
      : {}),

    // Add allowed domains only when the array is not empty.
    ...(options.includeDomains?.length
      ? { include_domains: options.includeDomains }
      : {}),

    // Add excluded domains only when the array is not empty.
    ...(options.excludeDomains?.length
      ? { exclude_domains: options.excludeDomains }
      : {}),
  };

  // Send the request through the shared authenticated transport.
  const responseData = await requestTavilyJson(
    TAVILY_SEARCH_PATH,
    {
      method: "POST",
      body: requestBody,
    },
  );

  // External JSON must remain unknown until validation succeeds.
  const validationResult =
    searchResponseSchema.safeParse(responseData);

  if (!validationResult.success) {
    throw new Error(
      "Tavily API returned an unexpected response format.",
    );
  }

  const data = validationResult.data;

  // Convert Tavily's response into consistent camelCase properties.
  return {
    query: data.query,
    answer: data.answer ?? null,
    results: data.results.map((result) => ({
      title: result.title,
      url: result.url,
      content: result.content,
      score: result.score,
      rawContent: result.raw_content ?? null,
      favicon: result.favicon ?? null,
    })),
    responseTime: String(data.response_time),
    requestId: data.request_id,
    creditsUsed: data.usage?.credits ?? null,
  };
}
