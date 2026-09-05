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
const TAVILY_CRAWL_PATH = "/crawl";

// Use a conservative project-level cap to control output and credit usage.
const MAX_CRAWL_RESULTS = 50;

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

// Validate one page returned by a crawl.
const crawlResultSchema = z
  .object({
    url: z.string(),
    raw_content: z.string(),
    images: z.array(imageSchema).optional(),
    favicon: z.string().nullable().optional(),
  })
  .passthrough();

// Validate the complete Crawl response used by this project.
const crawlResponseSchema = z
  .object({
    base_url: z.string(),
    results: z.array(crawlResultSchema),
    response_time: z.union([z.string(), z.number()]),
    usage: z
      .object({
        credits: z.number(),
      })
      .nullish(),
    request_id: z.string(),
  })
  .passthrough();

// Describe optional Tavily Crawl settings.
export interface TavilyCrawlOptions {
  instructions?: string;
  chunksPerSource?: number;
  maxDepth?: number;
  maxBreadth?: number;
  limit?: number;
  selectPaths?: string[];
  selectDomains?: string[];
  excludePaths?: string[];
  excludeDomains?: string[];
  allowExternal?: boolean;
  includeImages?: boolean;
  extractDepth?: TavilyExtractDepth;
  format?: TavilyContentFormat;
  includeFavicon?: boolean;
  timeout?: number;
}

// Describe one normalized crawled page.
export interface TavilyCrawlResult {
  url: string;
  rawContent: string;
  images: TavilyImage[];
  favicon: string | null;
}

// Describe the normalized Crawl response returned to the MCP layer.
export interface TavilyCrawlResponse {
  baseUrl: string;
  results: TavilyCrawlResult[];
  responseTime: string;
  creditsUsed: number | null;
  requestId: string;
}

/**
 * Traverse a website and extract content from matching pages.
 */
export async function crawlWithTavily(
  url: string,
  options: TavilyCrawlOptions = {},
): Promise<TavilyCrawlResponse> {
  // Normalize and validate the starting URL.
  const normalizedUrl = url.trim();
  let parsedUrl: URL;

  try {
    parsedUrl = new URL(normalizedUrl);
  } catch {
    throw new Error(`Invalid root URL: ${normalizedUrl}`);
  }

  if (!["http:", "https:"].includes(parsedUrl.protocol)) {
    throw new Error("The root URL must use HTTP or HTTPS.");
  }

  // Apply conservative defaults suitable for an MCP response.
  const maxDepth = options.maxDepth ?? 1;
  const maxBreadth = options.maxBreadth ?? 10;
  const limit = options.limit ?? 10;
  const timeout = options.timeout ?? 30;
  const normalizedInstructions = options.instructions?.trim();

  if (limit > MAX_CRAWL_RESULTS) {
    throw new Error(
      `limit cannot exceed ${MAX_CRAWL_RESULTS} in this MCP server.`,
    );
  }

  // Build the JSON body expected by Tavily Crawl.
  const requestBody = {
    url: normalizedUrl,
    max_depth: maxDepth,
    max_breadth: maxBreadth,
    limit,
    allow_external: options.allowExternal ?? false,
    include_images: options.includeImages ?? false,
    extract_depth: options.extractDepth ?? "basic",
    format: options.format ?? "markdown",
    include_favicon: options.includeFavicon ?? false,
    timeout,
    include_usage: true,

    // Natural-language instructions activate Tavily's guided mapping.
    ...(normalizedInstructions
      ? {
          instructions: normalizedInstructions,
          chunks_per_source: options.chunksPerSource ?? 3,
        }
      : {}),

    // Add optional regex filters only when they contain entries.
    ...(options.selectPaths?.length
      ? { select_paths: options.selectPaths }
      : {}),
    ...(options.selectDomains?.length
      ? { select_domains: options.selectDomains }
      : {}),
    ...(options.excludePaths?.length
      ? { exclude_paths: options.excludePaths }
      : {}),
    ...(options.excludeDomains?.length
      ? { exclude_domains: options.excludeDomains }
      : {}),
  };

  // Allow a small transport margin beyond Tavily's crawl timeout.
  const responseData = await requestTavilyJson(
    TAVILY_CRAWL_PATH,
    {
      method: "POST",
      body: requestBody,
      timeoutMs: (timeout + 5) * 1_000,
    },
  );

  // Never trust an external response before runtime validation.
  const validationResult =
    crawlResponseSchema.safeParse(responseData);

  if (!validationResult.success) {
    throw new Error(
      "Tavily API returned an unexpected Crawl response format.",
    );
  }

  const data = validationResult.data;

  // Convert Tavily's response into consistent camelCase properties.
  return {
    baseUrl: data.base_url,
    results: data.results.map((result) => ({
      url: result.url,
      rawContent: result.raw_content,
      images: (result.images ?? []).map(normalizeTavilyImage),
      favicon: result.favicon ?? null,
    })),
    responseTime: String(data.response_time),
    creditsUsed: data.usage?.credits ?? null,
    requestId: data.request_id,
  };
}
