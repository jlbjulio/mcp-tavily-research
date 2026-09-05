// Import the MCP server type without creating a runtime dependency.
import type { McpServer } from "@modelcontextprotocol/server";

// Import Zod to describe and validate tool arguments.
import * as z from "zod/v4";

// Import the endpoint-specific Crawl client.
import { crawlWithTavily } from "../clients/tavily-crawl-client.js";

// Import shared Tavily content option values.
import {
  TAVILY_CONTENT_FORMATS,
  TAVILY_EXTRACT_DEPTHS,
} from "../clients/tavily-content-options.js";

// Accept only absolute HTTP and HTTPS starting URLs.
const webUrlSchema = z
  .string()
  .trim()
  .url()
  .refine(
    (value) => ["http:", "https:"].includes(new URL(value).protocol),
    "URL must use HTTP or HTTPS.",
  );

// Reuse one schema for Tavily's optional regex filter arrays.
const regexListSchema = z
  .array(z.string().trim().min(1))
  .max(50)
  .optional();

/**
 * Register the Tavily Crawl tool on an MCP server.
 */
export function registerTavilyCrawlTool(
  server: McpServer,
): void {
  server.registerTool(
    "tavily_crawl",
    {
      title: "Tavily Website Crawl",
      description:
        "Traverse a website from one root URL and extract content from matching pages. Use for documentation sites or multi-page website analysis. Crawl combines mapping and extraction costs, so keep depth, breadth, and limit small unless broader traversal is necessary.",

      inputSchema: z.object({
        url: webUrlSchema.describe(
          "The absolute HTTP or HTTPS root URL where crawling begins.",
        ),

        instructions: z
          .string()
          .trim()
          .min(1)
          .max(2_000)
          .optional()
          .describe(
            "Optional natural-language guidance for finding relevant pages. Guided mapping costs more.",
          ),

        chunksPerSource: z
          .number()
          .int()
          .min(1)
          .max(5)
          .default(3)
          .describe(
            "Relevant chunks per page when instructions are provided. Ignored without instructions.",
          ),

        maxDepth: z
          .number()
          .int()
          .min(1)
          .max(5)
          .default(1)
          .describe("Maximum link depth from the root page."),

        maxBreadth: z
          .number()
          .int()
          .min(1)
          .max(500)
          .default(10)
          .describe("Maximum links followed from each level."),

        limit: z
          .number()
          .int()
          .min(1)
          .max(50)
          .default(10)
          .describe(
            "Maximum pages returned. This MCP applies a safety cap of 50.",
          ),

        selectPaths: regexListSchema.describe(
          "Optional regex patterns selecting URL paths to include.",
        ),

        selectDomains: regexListSchema.describe(
          "Optional regex patterns selecting domains or subdomains to include.",
        ),

        excludePaths: regexListSchema.describe(
          "Optional regex patterns for URL paths to exclude.",
        ),

        excludeDomains: regexListSchema.describe(
          "Optional regex patterns for domains or subdomains to exclude.",
        ),

        allowExternal: z
          .boolean()
          .default(false)
          .describe(
            "Allow pages from external domains in the final results. Disabled by default for scope control.",
          ),

        includeImages: z
          .boolean()
          .default(false)
          .describe("Include image URLs found on crawled pages."),

        extractDepth: z
          .enum(TAVILY_EXTRACT_DEPTHS)
          .default("basic")
          .describe(
            "Basic is cheaper; advanced improves extraction of tables and embedded content.",
          ),

        format: z
          .enum(TAVILY_CONTENT_FORMATS)
          .default("markdown")
          .describe("Return page content as Markdown or plain text."),

        includeFavicon: z
          .boolean()
          .default(false)
          .describe("Include the favicon URL for each page."),

        timeout: z
          .number()
          .min(10)
          .max(150)
          .default(30)
          .describe(
            "Maximum crawl time in seconds. Values above the MCP client's tool timeout may require client configuration.",
          ),
      }),

      // Crawling reads external pages without modifying them.
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
      chunksPerSource,
      maxDepth,
      maxBreadth,
      limit,
      selectPaths,
      selectDomains,
      excludePaths,
      excludeDomains,
      allowExternal,
      includeImages,
      extractDepth,
      format,
      includeFavicon,
      timeout,
    }) => {
      try {
        const result = await crawlWithTavily(url, {
          chunksPerSource,
          maxDepth,
          maxBreadth,
          limit,
          allowExternal,
          includeImages,
          extractDepth,
          format,
          includeFavicon,
          timeout,
          ...(instructions ? { instructions } : {}),
          ...(selectPaths ? { selectPaths } : {}),
          ...(selectDomains ? { selectDomains } : {}),
          ...(excludePaths ? { excludePaths } : {}),
          ...(excludeDomains ? { excludeDomains } : {}),
        });

        // Format every crawled page for text-only MCP clients.
        const formattedPages =
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
            : "No pages matched the crawl configuration.";

        return {
          content: [
            {
              type: "text",
              text: [
                "Crawled pages",
                "",
                formattedPages,
                "",
                "Metadata",
                "",
                `- Base URL: ${result.baseUrl}`,
                `- Pages returned: ${result.results.length}`,
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
