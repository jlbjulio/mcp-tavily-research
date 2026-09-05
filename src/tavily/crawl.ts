import * as z from "zod/v4";

import { requestTavilyJson } from "./api.js";

const TAVILY_CRAWL_PATH = "/crawl";
const MAX_CRAWL_RESULTS = 50;

export const TAVILY_EXTRACT_DEPTHS = [
  "basic",
  "advanced",
] as const;

export const TAVILY_CONTENT_FORMATS = [
  "markdown",
  "text",
] as const;

const tavilyImageSchema = z.union([
  z.string(),
  z
    .object({
      url: z.string(),
      description: z.string().nullable().optional(),
    })
    .passthrough(),
]);

type TavilyImage = z.infer<typeof tavilyImageSchema>;

export function getTavilyImageUrl(
  image: TavilyImage,
): string {
  return typeof image === "string" ? image : image.url;
}

const crawlResponseSchema = z
  .object({
    base_url: z.string(),
    results: z.array(
      z
        .object({
          url: z.string(),
          raw_content: z.string(),
          images: z.array(tavilyImageSchema).optional(),
          favicon: z.string().nullable().optional(),
        })
        .passthrough(),
    ),
    response_time: z.union([z.string(), z.number()]),
    usage: z
      .object({ credits: z.number() })
      .nullish(),
    request_id: z.string(),
  })
  .passthrough();

export interface TavilyCrawlOptions {
  instructions?: string;
  chunks_per_source?: number;
  max_depth?: number;
  max_breadth?: number;
  limit?: number;
  select_paths?: string[];
  select_domains?: string[];
  exclude_paths?: string[];
  exclude_domains?: string[];
  allow_external?: boolean;
  include_images?: boolean;
  extract_depth?: (typeof TAVILY_EXTRACT_DEPTHS)[number];
  format?: (typeof TAVILY_CONTENT_FORMATS)[number];
  include_favicon?: boolean;
  timeout?: number;
}

export type TavilyCrawlResponse = z.infer<
  typeof crawlResponseSchema
>;

export async function crawlWithTavily(
  url: string,
  options: TavilyCrawlOptions = {},
): Promise<TavilyCrawlResponse> {
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

  const limit = options.limit ?? 10;

  if (limit > MAX_CRAWL_RESULTS) {
    throw new Error(
      `limit cannot exceed ${MAX_CRAWL_RESULTS} in this MCP server.`,
    );
  }

  const instructions = options.instructions?.trim();
  const timeout = options.timeout ?? 30;

  const requestBody = {
    url: normalizedUrl,
    max_depth: options.max_depth ?? 1,
    max_breadth: options.max_breadth ?? 10,
    limit,
    allow_external: options.allow_external ?? false,
    include_images: options.include_images ?? false,
    extract_depth: options.extract_depth ?? "basic",
    format: options.format ?? "markdown",
    include_favicon: options.include_favicon ?? false,
    timeout,
    include_usage: true,
    ...(instructions
      ? {
          instructions,
          chunks_per_source:
            options.chunks_per_source ?? 3,
        }
      : {}),
    ...(options.select_paths?.length
      ? { select_paths: options.select_paths }
      : {}),
    ...(options.select_domains?.length
      ? { select_domains: options.select_domains }
      : {}),
    ...(options.exclude_paths?.length
      ? { exclude_paths: options.exclude_paths }
      : {}),
    ...(options.exclude_domains?.length
      ? { exclude_domains: options.exclude_domains }
      : {}),
  };

  return requestTavilyJson(
    TAVILY_CRAWL_PATH,
    {
      method: "POST",
      body: requestBody,
      timeout_ms: (timeout + 5) * 1_000,
    },
    crawlResponseSchema,
    "Crawl",
  );
}
