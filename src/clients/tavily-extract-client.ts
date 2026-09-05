// Import Zod to validate Tavily's external responses at runtime.
import * as z from "zod/v4";

// Import shared content options and normalization helpers.
import {
  normalizeTavilyImage,
  type TavilyContentFormat,
  type TavilyExtractDepth,
  type TavilyImage,
} from "./tavily-content-options.js";

// Import the shared authenticated HTTP transport.
import { requestTavilyJson } from "./tavily-http-client.js";

// Store the endpoint path used by the shared HTTP client.
const TAVILY_EXTRACT_PATH = "/extract";

// Tavily accepts at most twenty URLs in one Extract request.
const MAX_EXTRACT_URLS = 20;

// Accept both documented image representations from Tavily.
const imageSchema = z.union([
  z.string(),
  z
    .object({
      url: z.string(),
      description: z.string().nullable().optional(),
    })
    .passthrough(),
]);

// Validate one successfully extracted page.
const extractResultSchema = z
  .object({
    url: z.string(),
    raw_content: z.string(),
    images: z.array(imageSchema).optional(),
    favicon: z.string().nullable().optional(),
  })
  .passthrough();

// Validate one URL that Tavily could not extract.
const failedExtractResultSchema = z
  .object({
    url: z.string(),
    error: z.string().nullable().optional(),
  })
  .passthrough();

// Validate the complete Extract response used by this project.
const extractResponseSchema = z
  .object({
    results: z.array(extractResultSchema),
    failed_results: z.array(failedExtractResultSchema).optional(),
    response_time: z.union([z.string(), z.number()]),
    usage: z
      .object({
        credits: z.number(),
      })
      .nullish(),
    request_id: z.string(),
  })
  .passthrough();

// Describe optional Tavily Extract settings.
export interface TavilyExtractOptions {
  query?: string;
  chunksPerSource?: number;
  extractDepth?: TavilyExtractDepth;
  includeImages?: boolean;
  includeFavicon?: boolean;
  format?: TavilyContentFormat;
  timeout?: number;
}

// Describe one normalized successful extraction.
export interface TavilyExtractResult {
  url: string;
  rawContent: string;
  images: TavilyImage[];
  favicon: string | null;
}

// Describe one normalized failed extraction.
export interface TavilyFailedExtractResult {
  url: string;
  error: string | null;
}

// Describe the normalized Extract response returned to the MCP layer.
export interface TavilyExtractResponse {
  results: TavilyExtractResult[];
  failedResults: TavilyFailedExtractResult[];
  responseTime: string;
  creditsUsed: number | null;
  requestId: string;
}

/**
 * Extract readable content from one or more known web URLs.
 */
export async function extractWithTavily(
  urls: string[],
  options: TavilyExtractOptions = {},
): Promise<TavilyExtractResponse> {
  // Normalize copied URLs before validating or sending them.
  const normalizedUrls = urls.map((url) => url.trim());

  if (
    normalizedUrls.length < 1 ||
    normalizedUrls.length > MAX_EXTRACT_URLS
  ) {
    throw new Error(
      `urls must contain between 1 and ${MAX_EXTRACT_URLS} entries.`,
    );
  }

  // Accept only absolute HTTP and HTTPS URLs.
  for (const url of normalizedUrls) {
    let parsedUrl: URL;

    try {
      parsedUrl = new URL(url);
    } catch {
      throw new Error(`Invalid URL: ${url}`);
    }

    if (!["http:", "https:"].includes(parsedUrl.protocol)) {
      throw new Error(`Only HTTP and HTTPS URLs are supported: ${url}`);
    }
  }

  // Normalize the optional relevance query.
  const normalizedQuery = options.query?.trim();

  // Choose defaults that keep extraction predictable and inexpensive.
  const extractDepth = options.extractDepth ?? "basic";
  const format = options.format ?? "markdown";

  // Build the JSON body expected by Tavily Extract.
  const requestBody = {
    urls: normalizedUrls,
    extract_depth: extractDepth,
    include_images: options.includeImages ?? false,
    include_favicon: options.includeFavicon ?? false,
    format,
    include_usage: true,

    // Reranking and chunk limits apply only when a query is provided.
    ...(normalizedQuery
      ? {
          query: normalizedQuery,
          chunks_per_source: options.chunksPerSource ?? 3,
        }
      : {}),

    // Let Tavily apply its depth-specific timeout when omitted.
    ...(options.timeout !== undefined
      ? { timeout: options.timeout }
      : {}),
  };

  // Allow a small transport margin beyond Tavily's extraction timeout.
  const apiTimeoutSeconds =
    options.timeout ?? (extractDepth === "advanced" ? 30 : 10);

  const responseData = await requestTavilyJson(
    TAVILY_EXTRACT_PATH,
    {
      method: "POST",
      body: requestBody,
      timeoutMs: (apiTimeoutSeconds + 5) * 1_000,
    },
  );

  // Never trust an external response before runtime validation.
  const validationResult =
    extractResponseSchema.safeParse(responseData);

  if (!validationResult.success) {
    throw new Error(
      "Tavily API returned an unexpected Extract response format.",
    );
  }

  const data = validationResult.data;

  // Convert Tavily's response into consistent camelCase properties.
  return {
    results: data.results.map((result) => ({
      url: result.url,
      rawContent: result.raw_content,
      images: (result.images ?? []).map(normalizeTavilyImage),
      favicon: result.favicon ?? null,
    })),
    failedResults:
      data.failed_results?.map((result) => ({
        url: result.url,
        error: result.error ?? null,
      })) ?? [],
    responseTime: String(data.response_time),
    creditsUsed: data.usage?.credits ?? null,
    requestId: data.request_id,
  };
}
