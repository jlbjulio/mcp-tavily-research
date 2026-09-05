import type { McpServer } from "@modelcontextprotocol/server";
import * as z from "zod/v4";

import {
  crawlWithTavily,
  getTavilyImageUrl,
  TAVILY_CONTENT_FORMATS,
  TAVILY_EXTRACT_DEPTHS,
} from "../tavily/crawl.js";

const webUrlSchema = z
  .string()
  .trim()
  .url()
  .refine(
    (value) => ["http:", "https:"].includes(new URL(value).protocol),
    "URL must use HTTP or HTTPS.",
  );

const regexListSchema = z
  .array(z.string().trim().min(1))
  .max(50)
  .optional();

export function registerTavilyCrawlTool(
  server: McpServer,
): void {
  server.registerTool(
    "tavily_crawl",
    {
      title: "Tavily Website Crawl",
      description:
        "Crawl one website and extract content from matching pages. Keep the limit small to control cost.",
      inputSchema: z.object({
        url: webUrlSchema.describe("Root URL."),
        instructions: z
          .string()
          .trim()
          .min(1)
          .max(2_000)
          .optional()
          .describe(
            "Optional natural-language crawl instructions.",
          ),
        chunks_per_source: z
          .number()
          .int()
          .min(1)
          .max(5)
          .default(3)
          .describe(
            "Chunks per page when instructions are provided.",
          ),
        max_depth: z
          .number()
          .int()
          .min(1)
          .max(5)
          .default(1)
          .describe("Maximum crawl depth."),
        max_breadth: z
          .number()
          .int()
          .min(1)
          .max(500)
          .default(10)
          .describe("Maximum links followed per level."),
        limit: z
          .number()
          .int()
          .min(1)
          .max(50)
          .default(10)
          .describe("Maximum pages returned."),
        select_paths: regexListSchema.describe(
          "Regex patterns for paths to include.",
        ),
        select_domains: regexListSchema.describe(
          "Regex patterns for domains to include.",
        ),
        exclude_paths: regexListSchema.describe(
          "Regex patterns for paths to exclude.",
        ),
        exclude_domains: regexListSchema.describe(
          "Regex patterns for domains to exclude.",
        ),
        allow_external: z
          .boolean()
          .default(false)
          .describe("Include external-domain pages."),
        include_images: z
          .boolean()
          .default(false)
          .describe("Include images."),
        extract_depth: z
          .enum(TAVILY_EXTRACT_DEPTHS)
          .default("basic")
          .describe("Basic is cheaper; advanced extracts more."),
        format: z
          .enum(TAVILY_CONTENT_FORMATS)
          .default("markdown")
          .describe("Content format."),
        include_favicon: z
          .boolean()
          .default(false)
          .describe("Include favicons."),
        timeout: z
          .number()
          .min(10)
          .max(150)
          .default(30)
          .describe("Maximum crawl time in seconds."),
      }),
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: true,
      },
    },
    async ({
      url,
      instructions,
      chunks_per_source,
      max_depth,
      max_breadth,
      limit,
      select_paths,
      select_domains,
      exclude_paths,
      exclude_domains,
      allow_external,
      include_images,
      extract_depth,
      format,
      include_favicon,
      timeout,
    }) => {
      try {
        const result = await crawlWithTavily(url, {
          chunks_per_source,
          max_depth,
          max_breadth,
          limit,
          allow_external,
          include_images,
          extract_depth,
          format,
          include_favicon,
          timeout,
          ...(instructions ? { instructions } : {}),
          ...(select_paths ? { select_paths } : {}),
          ...(select_domains ? { select_domains } : {}),
          ...(exclude_paths ? { exclude_paths } : {}),
          ...(exclude_domains ? { exclude_domains } : {}),
        });

        const pages =
          result.results.length > 0
            ? result.results
                .map((page, index) => {
                  const sections = [
                    `[${index + 1}] ${page.url}`,
                    page.raw_content,
                  ];

                  if (page.images?.length) {
                    sections.push(
                      `Images:\n${page.images
                        .map(
                          (image) =>
                            `- ${getTavilyImageUrl(image)}`,
                        )
                        .join("\n")}`,
                    );
                  }

                  return sections.join("\n\n");
                })
                .join("\n\n---\n\n")
            : "No pages matched the crawl configuration.";

        return {
          content: [
            {
              type: "text",
              text: [
                "Crawled pages",
                "",
                pages,
                "",
                "Metadata",
                "",
                `- Base URL: ${result.base_url}`,
                `- Pages returned: ${result.results.length}`,
                `- Response time: ${result.response_time} seconds`,
                `- Credits used: ${result.usage?.credits ?? "unknown"}`,
                `- Request ID: ${result.request_id}`,
              ].join("\n"),
            },
          ],
          structuredContent: result,
        };
      } catch (error) {
        const message =
          error instanceof Error
            ? error.message
            : "Unknown Tavily Crawl error";

        return {
          content: [
            {
              type: "text",
              text: `Tavily crawl failed: ${message}`,
            },
          ],
          isError: true,
        };
      }
    },
  );
}
