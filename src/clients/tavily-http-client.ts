import type { ZodType } from "zod/v4";

import { getTavilyApiKey } from "../config.js";

const TAVILY_API_BASE_URL = "https://api.tavily.com";
const DEFAULT_TIMEOUT_MS = 30_000;

interface TavilyRequestOptions {
  method: "GET" | "POST";
  body?: unknown;
  timeout_ms?: number;
}

/**
 * Send an authenticated request and validate Tavily's JSON response.
 */
export async function requestTavilyJson<T>(
  path: string,
  options: TavilyRequestOptions,
  responseSchema: ZodType<T>,
  endpointName: string,
): Promise<T> {
  const headers: Record<string, string> = {
    Authorization: `Bearer ${getTavilyApiKey()}`,
  };

  if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
  }

  let response: Response;

  try {
    response = await fetch(`${TAVILY_API_BASE_URL}${path}`, {
      method: options.method,
      headers,
      signal: AbortSignal.timeout(
        options.timeout_ms ?? DEFAULT_TIMEOUT_MS,
      ),
      ...(options.body !== undefined
        ? { body: JSON.stringify(options.body) }
        : {}),
    });
  } catch (error) {
    const message =
      error instanceof Error
        ? error.message
        : "Unknown network error";

    throw new Error(`Tavily request failed: ${message}`, {
      cause: error,
    });
  }

  if (!response.ok) {
    const responseBody = await response.text();
    const detail =
      responseBody.slice(0, 500) || response.statusText;

    throw new Error(
      `Tavily API returned HTTP ${response.status}: ${detail}`,
    );
  }

  let responseData: unknown;

  try {
    responseData = (await response.json()) as unknown;
  } catch (error) {
    throw new Error(
      "Tavily API returned a response that was not valid JSON.",
      { cause: error },
    );
  }

  const validation = responseSchema.safeParse(responseData);

  if (!validation.success) {
    throw new Error(
      `Tavily API returned an unexpected ${endpointName} response.`,
    );
  }

  return validation.data;
}
