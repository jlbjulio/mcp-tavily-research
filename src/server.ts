// Import the high-level MCP server implementation.
import { McpServer } from "@modelcontextprotocol/server";

// Import each independent group of MCP tools.
import { registerTavilyCrawlTool } from "./tools/register-crawl-tool.js";
import { registerTavilyExtractTool } from "./tools/register-extract-tool.js";
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
        "Use tavily_search for focused lookups and source discovery. Use tavily_extract when specific page URLs are already known and their full content is needed. Use tavily_crawl for controlled multi-page traversal of one website. Use tavily_research only for broad multi-source analysis because it creates a billable asynchronous task. After tavily_research returns a request ID, poll that same task with tavily_research_status; never create a duplicate task.",
    },
  );

  // Attach all public tools before returning the configured server.
  registerTavilySearchTool(server);
  registerTavilyExtractTool(server);
  registerTavilyCrawlTool(server);
  registerTavilyResearchTools(server);

  return server;
}
