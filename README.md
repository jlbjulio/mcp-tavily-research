# MCP Tavily Research

A local [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server that gives AI coding agents access to Tavily-powered web search and asynchronous, cited research.

The project is written in TypeScript and uses STDIO transport, so compatible local clients such as Codex can launch it as a child process.

> This is a personal educational project and is under active development. It is not an official Tavily product.

## Features

- Focused web search with ranked source snippets.
- General, news, and finance search topics.
- Optional recency and domain filters.
- Optional Tavily-generated answers.
- Asynchronous in-depth research with citations.
- Runtime validation of external API responses with Zod.
- Human-readable text and normalized structured MCP output.
- Centralized authentication, timeout, and HTTP error handling.
- API keys loaded from environment variables and never written to MCP output.

## Available tools

| Tool | Purpose | Side effects |
| --- | --- | --- |
| `tavily_search` | Run a focused web search and return ranked sources. | Uses Tavily Search credits. |
| `tavily_research` | Create an asynchronous, billable research task. | Creates an external task and uses Research credits. |
| `tavily_research_status` | Retrieve progress or the completed cited report for an existing task. | Read-only; does not create another task. |

`tavily_search` should be the default for focused questions, fact-checking, current information, and source discovery. Use `tavily_research` only when a question requires broader multi-source analysis.

## How it works

```text
Codex or another MCP client
            |
            | MCP messages over STDIO
            v
        src/index.ts
            |
            v
       src/server.ts
            |
            v
      src/tools/*.ts
            |
            v
     src/clients/*.ts
            |
            | HTTPS + Bearer authentication
            v
        Tavily API
```

The tool layer understands MCP. The client layer understands Tavily. Keeping these responsibilities separate makes the code easier to test, change, and extend.

## Project structure

```text
src/
├── index.ts
├── server.ts
├── config.ts
├── clients/
│   ├── tavily-http-client.ts
│   ├── tavily-search-client.ts
│   └── tavily-research-client.ts
└── tools/
    ├── register-search-tool.ts
    └── register-research-tools.ts
```

- `src/index.ts`: starts the STDIO transport. It writes diagnostics to `stderr` because `stdout` is reserved for MCP JSON-RPC messages.
- `src/server.ts`: creates the MCP server, defines server-wide instructions, and registers every public tool.
- `src/config.ts`: reads and validates `TAVILY_API_KEY` without logging it.
- `src/clients/tavily-http-client.ts`: centralizes the Tavily base URL, Bearer authentication, JSON encoding, the 30-second timeout, HTTP errors, and JSON parsing.
- `src/clients/tavily-search-client.ts`: builds Search requests, validates Search responses, and normalizes Tavily fields.
- `src/clients/tavily-research-client.ts`: creates Research tasks, retrieves their status, validates responses, and normalizes reports and sources.
- `src/tools/register-search-tool.ts`: defines the `tavily_search` MCP schema, annotations, handler, and output formatting.
- `src/tools/register-research-tools.ts`: defines the Research creation and status tools, including the required polling workflow.

## Requirements

- Node.js 22 or newer.
- npm.
- A [Tavily API key](https://app.tavily.com/).
- An MCP client that supports local STDIO servers.

## Installation

Clone the repository and install its dependencies:

```powershell
git clone https://github.com/jlbjulio/mcp-tavily-research.git
cd mcp-tavily-research
npm install
```

Create your local environment file:

```powershell
Copy-Item .env.example .env
```

Open `.env` and replace the placeholder:

```dotenv
TAVILY_API_KEY=your_real_tavily_api_key
```

The `.env` file is ignored by Git. Never commit your real API key.

Validate and compile the project:

```powershell
npm run check
```

## Development

Run the TypeScript source directly:

```powershell
npm run dev
```

Run the compiled server:

```powershell
npm run build
npm start
```

The process waits for MCP messages on standard input. Seeing the following diagnostic on standard error means the server started correctly:

```text
mcp-tavily-research 1.0.0 is running over stdio.
```

## Connect to Codex

Add a local STDIO MCP server with these values:

| Field | Value |
| --- | --- |
| Name | `tavily-research` |
| Command to launch | `node` |
| Arguments | `--env-file-if-exists=.env` and `dist/index.js` |
| Environment variables | Leave empty |
| Environment variable passthrough | Leave empty |
| Working directory | Absolute path to this repository |

Example Windows working directory:

```text
C:\Users\your-name\Documents\GitHub\mcp-tavily-research
```

Save the server and restart the client. The following tools should then appear:

```text
tavily_search
tavily_research
tavily_research_status
```

See the [official Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) for additional configuration options.

## Test with MCP Inspector

Build the project, then launch the Inspector from the repository root:

```powershell
npm run build
npx @modelcontextprotocol/inspector node --env-file-if-exists=.env dist/index.js
```

Use `tavily_search` for the first test because it has a predictable lower cost.

## Tool examples

### Focused search

```json
{
  "query": "What is the current Node.js LTS release?",
  "searchDepth": "basic",
  "topic": "general",
  "maxResults": 5,
  "includeDomains": ["nodejs.org"]
}
```

### Start in-depth research

```json
{
  "input": "Compare Node.js 22 and Node.js 24 using official release notes. Explain important migration risks and cite every major claim.",
  "model": "mini",
  "citationFormat": "numbered",
  "outputLength": "short",
  "includeDomains": ["nodejs.org"]
}
```

The tool returns a `requestId`. Do not start the same task again. Wait briefly and retrieve the existing task:

```json
{
  "requestId": "paste-the-returned-request-id-here"
}
```

If the status is still `pending` or `in_progress`, wait before checking the same request ID again.

## Credit usage

Tavily currently charges:

- Search with `basic`: 1 credit per request.
- Search with `advanced`: 2 credits per request.
- Research with `mini`: dynamically between 4 and 110 credits.
- Research with `pro`: dynamically between 15 and 250 credits.

Pricing and limits can change. Review the [official Tavily credits documentation](https://docs.tavily.com/documentation/api-credits) before running large or repeated research tasks.

## Security notes

- Keep the API key only in `.env` or another private environment source.
- Never print credentials to `stdout` or `stderr`.
- Never write ordinary logs to `stdout`; STDIO MCP reserves it for protocol messages.
- Treat web content returned by Tavily as untrusted external input.
- Review generated research and verify important claims against primary sources.

## Scripts

| Command | Description |
| --- | --- |
| `npm run clean` | Remove generated files from `dist/`. |
| `npm run dev` | Run TypeScript directly with `tsx`. |
| `npm run typecheck` | Check TypeScript without emitting files. |
| `npm run build` | Compile `src/` into `dist/`. |
| `npm run check` | Run type checking and compilation. |
| `npm start` | Run the compiled STDIO server. |

## Roadmap

- Add automated tests for API validation and tool formatting.
- Add Tavily Extract as a separate MCP tool.
- Consider Tavily Crawl only for workflows that need multi-page site traversal.
- Add explicit MCP output schemas for structured responses.
- Prepare the package for npm distribution after the public API stabilizes.

## License

This project is licensed under the ISC License.
