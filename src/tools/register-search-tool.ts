import type { McpServer } from "@modelcontextprotocol/server";
import * as z from "zod/v4";

import {
  searchWithTavily,
  TAVILY_SEARCH_DEPTHS,
  TAVILY_SEARCH_TOPICS,
  TAVILY_TIME_RANGES,
} from "../clients/tavily-search-client.js";

export function registerTavilySearchTool(
  server: McpServer,
): void {
  server.registerTool(
    "tavily_search",
    {
      title: "Tavily Web Search",
      description:
        "Search the web for current information, fact-checking, news, and source discovery.",
      inputSchema: z.object({
        query: z
          .string()
          .trim()
          .min(1)
          .max(2_000)
          .describe("The question or search query."),
        search_depth: z
          .enum(TAVILY_SEARCH_DEPTHS)
          .default("basic")
          .describe(
            "Basic costs 1 credit; advanced costs 2 credits.",
          ),
        topic: z
          .enum(TAVILY_SEARCH_TOPICS)
          .default("general")
          .describe("Search category."),
        time_range: z
          .enum(TAVILY_TIME_RANGES)
          .optional()
          .describe("Optional publication recency filter."),
        max_results: z
          .number()
          .int()
          .min(1)
          .max(20)
          .default(8)
          .describe("Maximum number of sources."),
        include_domains: z
          .array(z.string().trim().min(1))
          .max(300)
          .optional()
          .describe("Optional domains to include."),
        exclude_domains: z
          .array(z.string().trim().min(1))
          .max(150)
          .optional()
          .describe("Optional domains to exclude."),
        include_answer: z
          .enum(["basic", "advanced"])
          .optional()
          .describe("Optionally include a Tavily-generated answer."),
      }),
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: true,
      },
    },
    async ({
      query,
      search_depth,
      topic,
      time_range,
      max_results,
      include_domains,
      exclude_domains,
      include_answer,
    }) => {
      try {
        const result = await searchWithTavily(query, {
          search_depth,
          topic,
          max_results,
          ...(time_range ? { time_range } : {}),
          ...(include_domains ? { include_domains } : {}),
          ...(exclude_domains ? { exclude_domains } : {}),
          ...(include_answer ? { include_answer } : {}),
        });

        const sources =
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

        const sections = result.answer
          ? [`Answer\n\n${result.answer}`]
          : [];

        sections.push(
          `Sources\n\n${sources}`,
          [
            "Metadata",
            "",
            `- Query: ${result.query}`,
            `- Response time: ${result.response_time} seconds`,
            `- Credits used: ${result.usage?.credits ?? "unknown"}`,
            `- Request ID: ${result.request_id}`,
          ].join("\n"),
        );

        return {
          content: [
            {
              type: "text",
              text: sections.join("\n\n"),
            },
          ],
          structuredContent: result,
        };
      } catch (error) {
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
