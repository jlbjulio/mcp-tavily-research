import * as z from "zod/v4";

import { requestTavilyJson } from "./api.js";

const TAVILY_EXTRACT_PATH = "/extract";
const MAX_EXTRACT_URLS = 20;

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

const extractResponseSchema = z
  .object({
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
    failed_results: z
      .array(
        z
          .object({
            url: z.string(),
            error: z.string().nullable().optional(),
          })
          .passthrough(),
      )
      .optional(),
    response_time: z.union([z.string(), z.number()]),
    usage: z
      .object({ credits: z.number() })
      .nullish(),
    request_id: z.string(),
  })
  .passthrough();

export interface TavilyExtractOptions {
  query?: string;
  chunks_per_source?: number;
  extract_depth?: (typeof TAVILY_EXTRACT_DEPTHS)[number];
  include_images?: boolean;
  include_favicon?: boolean;
  format?: (typeof TAVILY_CONTENT_FORMATS)[number];
  timeout?: number;
}

export type TavilyExtractResponse = z.infer<
  typeof extractResponseSchema
>;

export async function extractWithTavily(
  urls: string[],
  options: TavilyExtractOptions = {},
): Promise<TavilyExtractResponse> {
  const normalizedUrls = urls.map((url) => url.trim());

  if (
    normalizedUrls.length < 1 ||
    normalizedUrls.length > MAX_EXTRACT_URLS
  ) {
    throw new Error(
      `urls must contain between 1 and ${MAX_EXTRACT_URLS} entries.`,
    );
  }

  for (const url of normalizedUrls) {
    let parsedUrl: URL;

    try {
      parsedUrl = new URL(url);
    } catch {
      throw new Error(`Invalid URL: ${url}`);
    }

    if (!["http:", "https:"].includes(parsedUrl.protocol)) {
      throw new Error(
        `Only HTTP and HTTPS URLs are supported: ${url}`,
      );
    }
  }

  const query = options.query?.trim();
  const extract_depth = options.extract_depth ?? "basic";

  const requestBody = {
    urls: normalizedUrls,
    extract_depth,
    include_images: options.include_images ?? false,
    include_favicon: options.include_favicon ?? false,
    format: options.format ?? "markdown",
    include_usage: true,
    ...(query
      ? {
          query,
          chunks_per_source:
            options.chunks_per_source ?? 3,
        }
      : {}),
    ...(options.timeout !== undefined
      ? { timeout: options.timeout }
      : {}),
  };

  const apiTimeoutSeconds =
    options.timeout ??
    (extract_depth === "advanced" ? 30 : 10);

  return requestTavilyJson(
    TAVILY_EXTRACT_PATH,
    {
      method: "POST",
      body: requestBody,
      timeout_ms: (apiTimeoutSeconds + 5) * 1_000,
    },
    extractResponseSchema,
    "Extract",
  );
}
