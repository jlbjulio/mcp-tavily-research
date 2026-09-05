// Import the helper that serves MCP messages through stdin and stdout.
import { serveStdio } from "@modelcontextprotocol/server/stdio";

// Import the configured server factory and its identity.
import {
  createServer,
  SERVER_NAME,
  SERVER_VERSION,
} from "./server.js";

// Start a fresh MCP server for each STDIO client connection.
serveStdio(createServer, {
  // Write transport errors to stderr because stdout belongs to MCP.
  onerror(error) {
    console.error("MCP transport error:", error);
  },
});

// Diagnostic messages must also use stderr to protect JSON-RPC output.
console.error(
  `${SERVER_NAME} ${SERVER_VERSION} is running over stdio.`,
);
