// Import Zod to validate Tavily's external responses at runtime.
import * as z from "zod/v4";

// Import the shared authenticated HTTP transport.
import { requestTavilyJson } from "./tavily-http-client.js";

// Store the endpoint path used by the shared HTTP client.
const TAVILY_RESEARCH_PATH = "/research";

// List the Research models supported by Tavily.
export const TAVILY_RESEARCH_MODELS = [
  "mini",
  "pro",
  "auto",
] as const;

// List the supported citation formats.
export const TAVILY_CITATION_FORMATS = [
  "numbered",
  "mla",
  "apa",
  "chicago",
] as const;

// List the supported report lengths.
export const TAVILY_OUTPUT_LENGTHS = [
  "short",
  "standard",
  "long",
] as const;

// Derive TypeScript types from the constant arrays.
export type TavilyResearchModel =
  (typeof TAVILY_RESEARCH_MODELS)[number];

export type TavilyCitationFormat =
  (typeof TAVILY_CITATION_FORMATS)[number];

export type TavilyOutputLength =
  (typeof TAVILY_OUTPUT_LENGTHS)[number];

// Describe the optional settings accepted when creating research.
export interface TavilyResearchOptions {
  model?: TavilyResearchModel;
  citationFormat?: TavilyCitationFormat;
  outputLength?: TavilyOutputLength;
  includeDomains?: string[];
  excludeDomains?: string[];
}

// Validate the initial task returned by POST /research.
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

// Validate one source from a completed research report.
const researchSourceSchema = z
  .object({
    title: z.string(),
    url: z.string(),
    favicon: z.string().nullable().optional(),
  })
  .passthrough();

// Validate pending, completed, and failed status responses.
const researchStatusSchema = z
  .object({
    request_id: z.string(),
    created_at: z.string().optional(),
    status: z.string(),
    content: z
      .union([
        z.string(),
        z.record(z.string(), z.unknown()),
      ])
      .optional(),
    sources: z.array(researchSourceSchema).optional(),
    response_time: z.union([z.string(), z.number()]),
    error: z.string().optional(),
  })
  .passthrough();

// Describe the normalized result of creating a task.
export interface TavilyResearchTask {
  requestId: string;
  createdAt: string;
  status: string;
  input: string;
  model: string;
  responseTime: string;
}

// Describe one normalized research source.
export interface TavilyResearchSource {
  title: string;
  url: string;
  favicon: string | null;
}

// Describe a normalized status or completed report.
export interface TavilyResearchStatus {
  requestId: string;
  createdAt: string | null;
  status: string;
  content: string | Record<string, unknown> | null;
  sources: TavilyResearchSource[];
  responseTime: string;
  error: string | null;
}

/**
 * Create a new Tavily Research task.
 */
export async function startTavilyResearch(
  input: string,
  options: TavilyResearchOptions = {},
): Promise<TavilyResearchTask> {
  // Normalize and validate the research instruction.
  const normalizedInput = input.trim();

  if (!normalizedInput) {
    throw new Error("The research input cannot be empty.");
  }

  // Research supports at most twenty preferred domains.
  if (
    options.includeDomains &&
    options.includeDomains.length > 20
  ) {
    throw new Error(
      "includeDomains cannot contain more than 20 domains.",
    );
  }

  // Research supports at most twenty excluded domains.
  if (
    options.excludeDomains &&
    options.excludeDomains.length > 20
  ) {
    throw new Error(
      "excludeDomains cannot contain more than 20 domains.",
    );
  }

  // Build the task creation body expected by Tavily.
  const requestBody = {
    input: normalizedInput,

    // Mini is predictable and less expensive than Pro or Auto.
    model: options.model ?? "mini",

    // Polling is easier to integrate safely with MCP than streaming.
    stream: false,

    // Use familiar numbered citations unless requested otherwise.
    citation_format:
      options.citationFormat ?? "numbered",

    // Generate a standard-length report by default.
    output_length:
      options.outputLength ?? "standard",

    // Add preferred domains only when provided.
    ...(options.includeDomains?.length
      ? { include_domains: options.includeDomains }
      : {}),

    // Add blocked domains only when provided.
    ...(options.excludeDomains?.length
      ? { exclude_domains: options.excludeDomains }
      : {}),
  };

  // Create the asynchronous research task.
  const responseData = await requestTavilyJson(
    TAVILY_RESEARCH_PATH,
    {
      method: "POST",
      body: requestBody,
    },
  );

  // Validate the external response.
  const validationResult =
    researchTaskSchema.safeParse(responseData);

  if (!validationResult.success) {
    throw new Error(
      "Tavily API returned an unexpected research task format.",
    );
  }

  const data = validationResult.data;

  // Normalize Tavily's snake_case response.
  return {
    requestId: data.request_id,
    createdAt: data.created_at,
    status: data.status,
    input: data.input,
    model: data.model,
    responseTime: String(data.response_time),
  };
}

/**
 * Retrieve the current status or result of a Research task.
 */
export async function getTavilyResearch(
  requestId: string,
): Promise<TavilyResearchStatus> {
  // Remove accidental whitespace from copied request IDs.
  const normalizedRequestId = requestId.trim();

  if (!normalizedRequestId) {
    throw new Error("The research request ID cannot be empty.");
  }

  // Escape the ID before inserting it into a URL path.
  const encodedRequestId =
    encodeURIComponent(normalizedRequestId);

  // Retrieve the current task state.
  const responseData = await requestTavilyJson(
    `${TAVILY_RESEARCH_PATH}/${encodedRequestId}`,
    {
      method: "GET",
    },
  );

  // Validate pending, completed, or failed responses.
  const validationResult =
    researchStatusSchema.safeParse(responseData);

  if (!validationResult.success) {
    throw new Error(
      "Tavily API returned an unexpected research status format.",
    );
  }

  const data = validationResult.data;

  // Normalize fields that may be absent while work is in progress.
  return {
    requestId: data.request_id,
    createdAt: data.created_at ?? null,
    status: data.status,
    content: data.content ?? null,
    sources:
      data.sources?.map((source) => ({
        title: source.title,
        url: source.url,
        favicon: source.favicon ?? null,
      })) ?? [],
    responseTime: String(data.response_time),
    error: data.error ?? null,
  };
}
