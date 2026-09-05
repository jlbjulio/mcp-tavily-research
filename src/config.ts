// Store the environment variable name in one reusable constant.
const TAVILY_API_KEY_ENV_VAR = "TAVILY_API_KEY";

/**
 * Read and validate the Tavily API key.
 *
 * @returns The configured API key.
 * @throws An error when the environment variable is missing or empty.
 */
export function getTavilyApiKey(): string {
  // Read the variable and remove accidental surrounding whitespace.
  const apiKey = process.env[TAVILY_API_KEY_ENV_VAR]?.trim();

  // Stop the request early when no usable API key is configured.
  if (!apiKey) {
    throw new Error(
      `${TAVILY_API_KEY_ENV_VAR} is required. Configure it before calling a Tavily tool.`,
    );
  }

  // Return the validated value without logging or exposing it.
  return apiKey;
}