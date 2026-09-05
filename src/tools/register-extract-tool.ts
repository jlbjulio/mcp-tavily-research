// Import the MCP server type without creating a runtime dependency.
import type { McpServer } from "@modelcontextprotocol/server";

// Import Zod to describe and validate tool arguments.
import * as z from "zod/v4";

// Import shared Tavily content option values.
import {
  TAVILY_CONTENT_FORMATS,
  TAVILY_EXTRACT_DEPTHS,
} from "../clients/tavily-content-options.js";

// Import the endpoint-specific Extract client.
import { extractWithTavily } from "../clients/tavily-extract-client.js";

// Accept only absolute HTTP and HTTPS URLs at the MCP boundary.
const webUrlSchema = z
  .string()
  .trim()
  .url()
  .refine(
    (value) => ["http:", "https:"].includes(new URL(value).protocol),
    "URL must use HTTP or HTTPS.",
  );

/**
 * Register the Tavily Extract tool on an MCP server.
 */
export function registerTavilyExtractTool(
  server: McpServer,
): void {
  server.registerTool(
    "tavily_extract",
    {
      title: "Tavily Content Extract",
      description:
        "Extract readable content from one or more known web URLs. Use after discovering specific pages whose full content is needed. Supports up to 20 URLs per call and consumes Extract credits for successful pages.",

      inputSchema: z.object({
        urls: z
          .union([
            webUrlSchema,
            z.array(webUrlSchema).min(1).max(20),
          ])
          .describe(
            "One URL or an array of up to 20 URLs to extract.",
          ),

        query: z
          .string()
          .trim()
          .min(1)
          .max(2_000)
          .optional()
          .describe(
            "Optional intent used to rerank and return only relevant content chunks.",
          ),

        chunksPerSource: z
          .number()
          .int()
          .min(1)
          .max(5)
          .default(3)
          .describe(
            "Relevant chunks per URL when query is provided. Ignored without query.",
          ),

        extractDepth: z
          .enum(TAVILY_EXTRACT_DEPTHS)
          .default("basic")
          .describe(
            "Basic is cheaper; advanced improves extraction of tables and embedded content.",
          ),

        includeImages: z
          .boolean()
          .default(false)
          .describe("Include image URLs found on each page."),

        includeFavicon: z
          .boolean()
          .default(false)
          .describe("Include the favicon URL for each page."),

        format: z
          .enum(TAVILY_CONTENT_FORMATS)
          .default("markdown")
          .describe("Return extracted content as Markdown or plain text."),

        timeout: z
          .number()
          .min(1)
          .max(60)
          .optional()
          .describe(
            "Optional per-URL extraction timeout in seconds. Tavily applies a depth-specific default when omitted.",
          ),
      }),

      // Extraction reads external pages without modifying them.
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
      chunksPerSource,
      extractDepth,
      includeImages,
      includeFavicon,
      format,
      timeout,
    }) => {
      try {
        // Normalize a single URL and an array into the client shape.
        const normalizedUrls =
          typeof urls === "string" ? [urls] : urls;

        const result = await extractWithTavily(normalizedUrls, {
          chunksPerSource,
          extractDepth,
          includeImages,
          includeFavicon,
          format,
          ...(query ? { query } : {}),
          ...(timeout !== undefined ? { timeout } : {}),
        });

        // Format successfully extracted pages for text-only MCP clients.
        const formattedResults =
          result.results.length > 0
            ? result.results
                .map((page, index) => {
                  const sections = [
                    `[${index + 1}] ${page.url}`,
                    page.rawContent,
                  ];

                  if (page.images.length > 0) {
                    sections.push(
                      `Images:\n${page.images
                        .map((image) => `- ${image.url}`)
                        .join("\n")}`,
                    );
                  }

                  return sections.join("\n\n");
                })
                .join("\n\n---\n\n")
            : "No pages were extracted successfully.";

        // Preserve partial failures instead of hiding them.
        const formattedFailures =
          result.failedResults.length > 0
            ? result.failedResults
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
                formattedResults,
                "",
                "Failed pages",
                "",
                formattedFailures,
                "",
                "Metadata",
                "",
                `- Successful pages: ${result.results.length}`,
                `- Failed pages: ${result.failedResults.length}`,
                `- Response time: ${result.responseTime} seconds`,
                `- Credits used: ${result.creditsUsed ?? "unknown"}`,
                `- Request ID: ${result.requestId}`,
              ].join("\n"),
            },
          ],
          structuredContent: {
            ...result,
          },
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
