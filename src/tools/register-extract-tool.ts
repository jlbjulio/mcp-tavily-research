import type { McpServer } from "@modelcontextprotocol/server";
import * as z from "zod/v4";

import {
  extractWithTavily,
  getTavilyImageUrl,
  TAVILY_CONTENT_FORMATS,
  TAVILY_EXTRACT_DEPTHS,
} from "../tavily/extract.js";

const webUrlSchema = z
  .string()
  .trim()
  .url()
  .refine(
    (value) => ["http:", "https:"].includes(new URL(value).protocol),
    "URL must use HTTP or HTTPS.",
  );

export function registerTavilyExtractTool(
  server: McpServer,
): void {
  server.registerTool(
    "tavily_extract",
    {
      title: "Tavily Content Extract",
      description:
        "Extract readable content from up to 20 known web URLs.",
      inputSchema: z.object({
        urls: z
          .union([
            webUrlSchema,
            z.array(webUrlSchema).min(1).max(20),
          ])
          .describe("One URL or an array of up to 20 URLs."),
        query: z
          .string()
          .trim()
          .min(1)
          .max(2_000)
          .optional()
          .describe("Optional intent for reranking content."),
        chunks_per_source: z
          .number()
          .int()
          .min(1)
          .max(5)
          .default(3)
          .describe(
            "Chunks per URL when query is provided.",
          ),
        extract_depth: z
          .enum(TAVILY_EXTRACT_DEPTHS)
          .default("basic")
          .describe("Basic is cheaper; advanced extracts more."),
        include_images: z
          .boolean()
          .default(false)
          .describe("Include images."),
        include_favicon: z
          .boolean()
          .default(false)
          .describe("Include favicons."),
        format: z
          .enum(TAVILY_CONTENT_FORMATS)
          .default("markdown")
          .describe("Content format."),
        timeout: z
          .number()
          .min(1)
          .max(60)
          .optional()
          .describe("Extraction timeout in seconds."),
      }),
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: true,
      },
    },
    async ({
      urls,
      query,
      chunks_per_source,
      extract_depth,
      include_images,
      include_favicon,
      format,
      timeout,
    }) => {
      try {
        const result = await extractWithTavily(
          typeof urls === "string" ? [urls] : urls,
          {
            chunks_per_source,
            extract_depth,
            include_images,
            include_favicon,
            format,
            ...(query ? { query } : {}),
            ...(timeout !== undefined ? { timeout } : {}),
          },
        );

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
            : "No pages were extracted successfully.";

        const failed_results = result.failed_results ?? [];
        const failures =
          failed_results.length > 0
            ? failed_results
                .map(
                  (failure) =>
                    `- ${failure.url}: ${failure.error ?? "No error detail was provided."}`,
                )
                .join("\n")
            : "None";

        return {
          content: [
            {
              type: "text",
              text: [
                "Extracted pages",
                "",
                pages,
                "",
                "Failed pages",
                "",
                failures,
                "",
                "Metadata",
                "",
                `- Successful pages: ${result.results.length}`,
                `- Failed pages: ${failed_results.length}`,
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
            : "Unknown Tavily Extract error";

        return {
          content: [
            {
              type: "text",
              text: `Tavily extraction failed: ${message}`,
            },
          ],
          isError: true,
        };
      }
    },
  );
}
