import * as z from "zod/v4";

import { requestTavilyJson } from "./tavily-http-client.js";

const TAVILY_RESEARCH_PATH = "/research";

export const TAVILY_RESEARCH_MODELS = [
  "mini",
  "pro",
  "auto",
] as const;

export const TAVILY_CITATION_FORMATS = [
  "numbered",
  "mla",
  "apa",
  "chicago",
] as const;

export const TAVILY_OUTPUT_LENGTHS = [
  "short",
  "standard",
  "long",
] as const;

export interface TavilyResearchOptions {
  model?: (typeof TAVILY_RESEARCH_MODELS)[number];
  citation_format?: (typeof TAVILY_CITATION_FORMATS)[number];
  output_length?: (typeof TAVILY_OUTPUT_LENGTHS)[number];
  include_domains?: string[];
  exclude_domains?: string[];
}

const researchTaskSchema = z
  .object({
    request_id: z.string(),
    created_at: z.string(),
    status: z.string(),
    input: z.string(),
    model: z.string(),
    response_time: z.union([z.string(), z.number()]),
  })
  .passthrough();

const researchSourceSchema = z
  .object({
    title: z.string(),
    url: z.string(),
    favicon: z.string().nullable().optional(),
  })
  .passthrough();

const researchStatusSchema = z
  .object({
    request_id: z.string(),
    created_at: z.string().nullish(),
    status: z.string(),
    content: z
      .union([
        z.string(),
        z.record(z.string(), z.unknown()),
      ])
      .nullish(),
    sources: z.array(researchSourceSchema).optional(),
    response_time: z.union([z.string(), z.number()]),
    error: z.string().nullish(),
  })
  .passthrough();

export type TavilyResearchTask = z.infer<
  typeof researchTaskSchema
>;

export type TavilyResearchStatus = z.infer<
  typeof researchStatusSchema
>;

export async function startTavilyResearch(
  input: string,
  options: TavilyResearchOptions = {},
): Promise<TavilyResearchTask> {
  const normalizedInput = input.trim();

  if (!normalizedInput) {
    throw new Error("The research input cannot be empty.");
  }

  if (
    options.include_domains &&
    options.include_domains.length > 20
  ) {
    throw new Error(
      "include_domains cannot contain more than 20 domains.",
    );
  }

  if (
    options.exclude_domains &&
    options.exclude_domains.length > 20
  ) {
    throw new Error(
      "exclude_domains cannot contain more than 20 domains.",
    );
  }

  const requestBody = {
    input: normalizedInput,
    model: options.model ?? "mini",
    stream: false,
    citation_format:
      options.citation_format ?? "numbered",
    output_length: options.output_length ?? "standard",
    ...(options.include_domains?.length
      ? { include_domains: options.include_domains }
      : {}),
    ...(options.exclude_domains?.length
      ? { exclude_domains: options.exclude_domains }
      : {}),
  };

  return requestTavilyJson(
    TAVILY_RESEARCH_PATH,
    {
      method: "POST",
      body: requestBody,
    },
    researchTaskSchema,
    "Research task",
  );
}

export async function getTavilyResearch(
  request_id: string,
): Promise<TavilyResearchStatus> {
  const normalizedRequestId = request_id.trim();

  if (!normalizedRequestId) {
    throw new Error("The research request ID cannot be empty.");
  }

  return requestTavilyJson(
    `${TAVILY_RESEARCH_PATH}/${encodeURIComponent(normalizedRequestId)}`,
    { method: "GET" },
    researchStatusSchema,
    "Research status",
  );
}
