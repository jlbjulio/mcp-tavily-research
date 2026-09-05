// Import the MCP server type without creating a runtime dependency.
import type { McpServer } from "@modelcontextprotocol/server";

// Import Zod to describe and validate tool arguments.
import * as z from "zod/v4";

// Import the Search client and its supported option values.
import {
  searchWithTavily,
  TAVILY_SEARCH_DEPTHS,
  TAVILY_SEARCH_TOPICS,
  TAVILY_TIME_RANGES,
} from "../clients/tavily-search-client.js";

/**
 * Register the Tavily Search tool on an MCP server.
 */
export function registerTavilySearchTool(
  server: McpServer,
): void {
  server.registerTool(
    "tavily_search",
    {
      title: "Tavily Web Search",
      description:
        "Search the web for current information and relevant sources. Use for focused questions, fact-checking, news, and source discovery.",

      // Define every argument that an MCP client may provide.
      inputSchema: z.object({
        query: z
          .string()
          .trim()
          .min(1)
          .max(2_000)
          .describe("The question or search query to investigate."),

        searchDepth: z
          .enum(TAVILY_SEARCH_DEPTHS)
          .default("basic")
          .describe(
            "Search depth. Basic costs 1 credit; advanced costs 2 credits and improves relevance.",
          ),

        topic: z
          .enum(TAVILY_SEARCH_TOPICS)
          .default("general")
          .describe(
            "Search category: general, news, or finance.",
          ),

        timeRange: z
          .enum(TAVILY_TIME_RANGES)
          .optional()
          .describe("Optional publication recency filter."),

        maxResults: z
          .number()
          .int()
          .min(1)
          .max(20)
          .default(8)
          .describe("Maximum number of sources to return."),

        includeDomains: z
          .array(z.string().trim().min(1))
          .max(300)
          .optional()
          .describe(
            "Optional domains that should be included in the search.",
          ),

        excludeDomains: z
          .array(z.string().trim().min(1))
          .max(150)
          .optional()
          .describe(
            "Optional domains that should be excluded from the search.",
          ),

        includeAnswer: z
          .enum(["basic", "advanced"])
          .optional()
          .describe(
            "Optionally request an additional Tavily-generated answer.",
          ),
      }),

      // Search reads the open web but does not create external resources.
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: true,
      },
    },

    // Execute the tool after MCP validates all arguments.
    async ({
      query,
      searchDepth,
      topic,
      timeRange,
      maxResults,
      includeDomains,
      excludeDomains,
      includeAnswer,
    }) => {
      try {
        // Call the endpoint-specific Tavily Search client.
        const result = await searchWithTavily(query, {
          searchDepth,
          topic,
          maxResults,

          // Avoid explicitly assigning undefined to optional properties.
          ...(timeRange ? { timeRange } : {}),
          ...(includeDomains ? { includeDomains } : {}),
          ...(excludeDomains ? { excludeDomains } : {}),
          ...(includeAnswer ? { includeAnswer } : {}),
        });

        // Convert every source into readable text for MCP clients.
        const formattedSources =
          result.results.length > 0
            ? result.results
                .map((source, index) =>
                  [
                    `[${index + 1}] ${source.title}`,
                    `URL: ${source.url}`,
                    `Relevance score: ${source.score.toFixed(3)}`,
                    source.content,
                  ].join("\n"),
                )
                .join("\n\n")
            : "No sources were returned.";

        // Build separate sections to keep the response readable.
        const sections: string[] = [];

        if (result.answer) {
          sections.push(`Answer\n\n${result.answer}`);
        }

        sections.push(`Sources\n\n${formattedSources}`);

        sections.push(
          [
            "Metadata",
            "",
            `- Query: ${result.query}`,
            `- Response time: ${result.responseTime} seconds`,
            `- Credits used: ${result.creditsUsed ?? "unknown"}`,
            `- Request ID: ${result.requestId}`,
          ].join("\n"),
        );

        return {
          // Text content remains compatible with every MCP client.
          content: [
            {
              type: "text",
              text: sections.join("\n\n"),
            },
          ],

          // Structured content preserves every normalized result field.
          structuredContent: {
            ...result,
          },
        };
      } catch (error) {
        // Convert client failures into an MCP tool error.
        const message =
          error instanceof Error
            ? error.message
            : "Unknown Tavily error";

        return {
          content: [
            {
              type: "text",
              text: `Tavily search failed: ${message}`,
            },
          ],
          isError: true,
        };
      }
    },
  );
}
