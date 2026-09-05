// Import the configuration helper that provides the Tavily API key.
import { getTavilyApiKey } from "../config.js";

// Keep Tavily's base URL in one place for every API client.
const TAVILY_API_BASE_URL = "https://api.tavily.com";

// Prevent individual HTTP operations from waiting indefinitely.
const REQUEST_TIMEOUT_MS = 30_000;

// Describe the small set of HTTP options needed by this project.
interface TavilyRequestOptions {
  method: "GET" | "POST";
  body?: unknown;
  timeoutMs?: number;
}

/**
 * Send an authenticated JSON request to the Tavily API.
 *
 * Endpoint-specific clients remain responsible for validating the
 * returned data because every Tavily endpoint has a different schema.
 */
export async function requestTavilyJson(
  path: string,
  options: TavilyRequestOptions,
): Promise<unknown> {
  // Read the secret only when an API request is about to run.
  const apiKey = getTavilyApiKey();

  // Create the authentication header required by every Tavily request.
  const headers: Record<string, string> = {
    Authorization: `Bearer ${apiKey}`,
  };

  // JSON content headers are necessary only when sending a body.
  if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
  }

  // Build the request without explicitly assigning undefined fields.
  const requestOptions: RequestInit = {
    method: options.method,
    headers,
    signal: AbortSignal.timeout(
      options.timeoutMs ?? REQUEST_TIMEOUT_MS,
    ),
    ...(options.body !== undefined
      ? { body: JSON.stringify(options.body) }
      : {}),
  };

  let response: Response;

  try {
    // Node.js provides fetch globally, so no additional HTTP library is needed.
    response = await fetch(
      `${TAVILY_API_BASE_URL}${path}`,
      requestOptions,
    );
  } catch (error) {
    // Convert network and timeout failures into a readable project error.
    const message =
      error instanceof Error
        ? error.message
        : "Unknown network error";

    throw new Error(`Tavily request failed: ${message}`, {
      cause: error,
    });
  }

  // Reject unsuccessful HTTP responses before parsing a success schema.
  if (!response.ok) {
    const responseBody = await response.text();
    const detail =
      responseBody.slice(0, 500) || response.statusText;

    throw new Error(
      `Tavily API returned HTTP ${response.status}: ${detail}`,
    );
  }

  try {
    // Keep external data unknown until an endpoint client validates it.
    return (await response.json()) as unknown;
  } catch (error) {
    throw new Error(
      "Tavily API returned a response that was not valid JSON.",
      { cause: error },
    );
  }
}
