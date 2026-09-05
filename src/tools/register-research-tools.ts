import type { McpServer } from "@modelcontextprotocol/server";
import * as z from "zod/v4";

import {
  getTavilyResearch,
  startTavilyResearch,
  TAVILY_CITATION_FORMATS,
  TAVILY_OUTPUT_LENGTHS,
  TAVILY_RESEARCH_MODELS,
} from "../clients/tavily-research-client.js";

export function registerTavilyResearchTools(
  server: McpServer,
): void {
  server.registerTool(
    "tavily_research",
    {
      title: "Start Tavily Research",
      description:
        "Start a billable, asynchronous multi-source research task. Poll the returned request_id with tavily_research_status.",
      inputSchema: z.object({
        input: z
          .string()
          .trim()
          .min(1)
          .max(20_000)
          .describe("Detailed research instructions."),
        model: z
          .enum(TAVILY_RESEARCH_MODELS)
          .default("mini")
          .describe("Mini is cheaper; Pro is more comprehensive."),
        citation_format: z
          .enum(TAVILY_CITATION_FORMATS)
          .default("numbered")
          .describe("Citation style."),
        output_length: z
          .enum(TAVILY_OUTPUT_LENGTHS)
          .default("standard")
          .describe("Report length."),
        include_domains: z
          .array(z.string().trim().min(1))
          .max(20)
          .optional()
          .describe("Domains to prioritize."),
        exclude_domains: z
          .array(z.string().trim().min(1))
          .max(20)
          .optional()
          .describe("Domains to exclude."),
      }),
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: true,
      },
    },
    async ({
      input,
      model,
      citation_format,
      output_length,
      include_domains,
      exclude_domains,
    }) => {
      try {
        const task = await startTavilyResearch(input, {
          model,
          citation_format,
          output_length,
          ...(include_domains ? { include_domains } : {}),
          ...(exclude_domains ? { exclude_domains } : {}),
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
                `- Request ID: ${task.request_id}`,
                `- Created at: ${task.created_at}`,
                "",
                "Wait a few seconds, then call tavily_research_status with this request ID. Do not create the task again.",
              ].join("\n"),
            },
          ],
          structuredContent: task,
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

  server.registerTool(
    "tavily_research_status",
    {
      title: "Get Tavily Research Status",
      description:
        "Get the status or result of an existing Research task. Never start a duplicate task.",
      inputSchema: z.object({
        request_id: z
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
    async ({ request_id }) => {
      try {
        const result = await getTavilyResearch(request_id);

        if (result.status === "failed") {
          return {
            content: [
              {
                type: "text",
                text: [
                  "Tavily Research task failed.",
                  "",
                  `- Request ID: ${result.request_id}`,
                  `- Error: ${result.error ?? "No error detail was provided."}`,
                ].join("\n"),
              },
            ],
            structuredContent: result,
            isError: true,
          };
        }

        if (result.status !== "completed") {
          return {
            content: [
              {
                type: "text",
                text: [
                  "Tavily Research is still in progress.",
                  "",
                  `- Status: ${result.status}`,
                  `- Request ID: ${result.request_id}`,
                  "",
                  "Wait approximately five seconds before checking again.",
                ].join("\n"),
              },
            ],
            structuredContent: result,
          };
        }

        const report =
          typeof result.content === "string"
            ? result.content
            : result.content
              ? JSON.stringify(result.content, null, 2)
              : "No report content was returned.";

        const sources = result.sources ?? [];
        const formattedSources =
          sources.length > 0
            ? sources
                .map(
                  (source, index) =>
                    `[${index + 1}] ${source.title}\n${source.url}`,
                )
                .join("\n\n")
            : "No sources were returned.";

        const alreadyHasSources =
          typeof result.content === "string" &&
          /^\s{0,3}(?:#{1,6}\s*)?sources:?\s*$/im.test(
            result.content,
          );

        const sections = [report];

        if (!alreadyHasSources) {
          sections.push(`Sources\n\n${formattedSources}`);
        }

        sections.push(
          [
            "Metadata",
            "",
            `- Status: ${result.status}`,
            `- Request ID: ${result.request_id}`,
            `- Response time: ${result.response_time} seconds`,
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
