// Import the MCP server type without generating a runtime import.
import type { McpServer } from "@modelcontextprotocol/server";

// Import Zod to describe and validate tool arguments.
import * as z from "zod/v4";

// Import the Research client and its supported option values.
import {
  getTavilyResearch,
  startTavilyResearch,
  TAVILY_CITATION_FORMATS,
  TAVILY_OUTPUT_LENGTHS,
  TAVILY_RESEARCH_MODELS,
} from "../clients/tavily-research-client.js";

/**
 * Register the Tavily Research tools on an MCP server.
 */
export function registerTavilyResearchTools(
  server: McpServer,
): void {
  // Register the tool that creates a billable Research task.
  server.registerTool(
    "tavily_research",
    {
      title: "Start Tavily Research",
      description:
        "Start an in-depth, billable research task that performs multiple searches and produces a cited report. Use only for complex questions requiring broad analysis. After starting, use tavily_research_status with the returned request ID.",

      inputSchema: z.object({
        input: z
          .string()
          .trim()
          .min(1)
          .max(20_000)
          .describe(
            "A detailed description of the topic, questions, scope, and desired research outcome.",
          ),

        model: z
          .enum(TAVILY_RESEARCH_MODELS)
          .default("mini")
          .describe(
            "Research model. Mini is cheaper; Pro is more comprehensive; Auto lets Tavily choose.",
          ),

        citationFormat: z
          .enum(TAVILY_CITATION_FORMATS)
          .default("numbered")
          .describe(
            "Citation style for the generated report.",
          ),

        outputLength: z
          .enum(TAVILY_OUTPUT_LENGTHS)
          .default("standard")
          .describe(
            "Target length of the generated report.",
          ),

        includeDomains: z
          .array(z.string().trim().min(1))
          .max(20)
          .optional()
          .describe(
            "Optional domains Tavily should prioritize as sources.",
          ),

        excludeDomains: z
          .array(z.string().trim().min(1))
          .max(20)
          .optional()
          .describe(
            "Optional domains Tavily must exclude from the report.",
          ),
      }),

      annotations: {
        readOnlyHint: false,
        destructiveHint: false,

        // Starting the same task twice creates and charges twice.
        idempotentHint: false,

        // The task searches and reads external web sources.
        openWorldHint: true,
      },
    },

    async ({
      input,
      model,
      citationFormat,
      outputLength,
      includeDomains,
      excludeDomains,
    }) => {
      try {
        // Create the asynchronous Tavily Research task.
        const task = await startTavilyResearch(input, {
          model,
          citationFormat,
          outputLength,
          ...(includeDomains ? { includeDomains } : {}),
          ...(excludeDomains ? { excludeDomains } : {}),
        });

        return {
          content: [
            {
              type: "text",
              text: [
                "Tavily Research task created.",
                "",
                `- Status: ${task.status}`,
                `- Model: ${task.model}`,
                `- Request ID: ${task.requestId}`,
                `- Created at: ${task.createdAt}`,
                "",
                "Next action: wait a few seconds, then call tavily_research_status with this request ID. Do not create the task again.",
              ].join("\n"),
            },
          ],
          structuredContent: {
            ...task,
          },
        };
      } catch (error) {
        const message =
          error instanceof Error
            ? error.message
            : "Unknown Tavily Research error";

        return {
          content: [
            {
              type: "text",
              text: `Failed to start Tavily Research: ${message}`,
            },
          ],
          isError: true,
        };
      }
    },
  );

  // Register the tool that retrieves progress or a completed report.
  server.registerTool(
    "tavily_research_status",
    {
      title: "Get Tavily Research Status",
      description:
        "Retrieve the status or completed report of an existing Tavily Research task. If the status is in_progress, wait before calling this tool again. Never start a duplicate task.",

      inputSchema: z.object({
        requestId: z
          .string()
          .trim()
          .min(1)
          .describe(
            "The request ID returned by tavily_research.",
          ),
      }),

      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: true,
      },
    },

    async ({ requestId }) => {
      try {
        // Retrieve the latest state from Tavily.
        const result = await getTavilyResearch(requestId);

        // Treat an explicitly failed task as an MCP tool error.
        if (result.status === "failed") {
          return {
            content: [
              {
                type: "text",
                text: [
                  "Tavily Research task failed.",
                  "",
                  `- Request ID: ${result.requestId}`,
                  `- Error: ${result.error ?? "No error detail was provided."}`,
                ].join("\n"),
              },
            ],
            structuredContent: {
              ...result,
            },
            isError: true,
          };
        }

        // A non-completed status means Tavily is still working.
        if (result.status !== "completed") {
          return {
            content: [
              {
                type: "text",
                text: [
                  "Tavily Research is still in progress.",
                  "",
                  `- Status: ${result.status}`,
                  `- Request ID: ${result.requestId}`,
                  "",
                  "Wait approximately five seconds before checking again.",
                ].join("\n"),
              },
            ],
            structuredContent: {
              ...result,
            },
          };
        }

        // Convert string or structured report content into readable text.
        const formattedContent =
          typeof result.content === "string"
            ? result.content
            : result.content
              ? JSON.stringify(result.content, null, 2)
              : "No report content was returned.";

        // Format the sources separately for clients that prefer text.
        const formattedSources =
          result.sources.length > 0
            ? result.sources
                .map(
                  (source, index) =>
                    `[${index + 1}] ${source.title}\n${source.url}`,
                )
                .join("\n\n")
            : "No sources were returned.";

        // Detect whether Tavily already added a Sources section.
        const contentAlreadyHasSources =
          typeof result.content === "string" &&
          /^\s{0,3}(?:#{1,6}\s*)?sources:?\s*$/im.test(
            result.content,
          );

        // Build the text response one section at a time.
        const sections = [formattedContent];

        // Avoid showing the same source list twice.
        if (!contentAlreadyHasSources) {
          sections.push(`Sources\n\n${formattedSources}`);
        }

        // Always place task metadata at the end.
        sections.push(
          [
            "Metadata",
            "",
            `- Status: ${result.status}`,
            `- Request ID: ${result.requestId}`,
            `- Response time: ${result.responseTime} seconds`,
          ].join("\n"),
        );

        return {
          content: [
            {
              type: "text",
              text: sections.join("\n\n"),
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
            : "Unknown Tavily Research error";

        return {
          content: [
            {
              type: "text",
              text: `Failed to retrieve Tavily Research: ${message}`,
            },
          ],
          isError: true,
        };
      }
    },
  );
}
