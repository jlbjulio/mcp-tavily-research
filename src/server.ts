// Import the high-level MCP server implementation.
import { McpServer } from "@modelcontextprotocol/server";

// Import each independent group of MCP tools.
import { registerTavilyResearchTools } from "./tools/register-research-tools.js";
import { registerTavilySearchTool } from "./tools/register-search-tool.js";

// Keep the server identity in one reusable location.
export const SERVER_NAME = "mcp-tavily-research";
export const SERVER_VERSION = "1.0.0";

/**
 * Create and configure one MCP server instance.
 */
export function createServer(): McpServer {
  // Describe the server and its cross-tool workflow to MCP clients.
  const server = new McpServer(
    {
      name: SERVER_NAME,
      version: SERVER_VERSION,
    },
    {
      instructions:
        "Use tavily_search for focused lookups, current information, fact-checking, and source discovery. Use tavily_research only for complex questions requiring broad multi-source analysis because it creates a billable asynchronous task. After tavily_research returns a request ID, wait before calling tavily_research_status. Poll the same task until it completes; never create a duplicate task.",
    },
  );

  // Attach all public tools before returning the configured server.
  registerTavilySearchTool(server);
  registerTavilyResearchTools(server);

  return server;
}
