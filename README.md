# MCP Tavily Research

A local [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server that gives AI coding agents access to Tavily Search, Extract, Crawl, and asynchronous Research.

The project is written in TypeScript and uses STDIO transport, so compatible local clients such as Codex can launch it as a child process.

> This is a personal educational project under active development. It is not an official Tavily product.

## Features

- Focused web search with ranked source snippets.
- Content extraction from up to 20 known URLs per request.
- Controlled multi-page website crawling with path and domain filters.
- Asynchronous in-depth research with citations and status polling.
- General, news, and finance search topics.
- Optional recency and domain filters.
- Runtime validation of external API responses with Zod.
- Human-readable text plus normalized structured MCP output.
- Centralized authentication, request timeouts, and HTTP error handling.
- API keys loaded from environment variables and never written to MCP output.

## Available tools

| Tool | Purpose | Side effects |
| --- | --- | --- |
| `tavily_search` | Run a focused web search and return ranked sources. | Uses Tavily Search credits. |
| `tavily_extract` | Extract readable content from one or more known URLs. | Uses Extract credits for successful URLs. |
| `tavily_crawl` | Traverse one website and extract matching pages. | Uses mapping and extraction credits. |
| `tavily_research` | Create an asynchronous, billable research task. | Creates an external task and uses Research credits. |
| `tavily_research_status` | Retrieve progress or the completed cited report for an existing task. | Read-only; does not create another task. |

Choose the smallest tool that fits the job:

- Use `tavily_search` to find facts, current information, or promising source URLs.
- Use `tavily_extract` when the URLs are already known and their full content is needed.
- Use `tavily_crawl` when multiple pages from one website must be traversed.
- Use `tavily_research` only for broad multi-source analysis. Poll the returned request ID with `tavily_research_status`; do not create a duplicate task.

## How it works

```text
Codex or another MCP client
            |
            | MCP JSON-RPC over STDIO
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

The tool layer understands MCP: it defines public names, input schemas, annotations, error responses, and output formatting. The client layer understands Tavily: it builds API requests, validates external responses, and converts snake_case API fields into predictable camelCase objects.

Keeping those responsibilities separate prevents protocol code from becoming coupled to one HTTP endpoint and makes future testing or extension easier.

## Project structure

```text
src/
|-- index.ts
|-- server.ts
|-- config.ts
|-- clients/
|   |-- tavily-http-client.ts
|   |-- tavily-content-options.ts
|   |-- tavily-search-client.ts
|   |-- tavily-extract-client.ts
|   |-- tavily-crawl-client.ts
|   `-- tavily-research-client.ts
`-- tools/
    |-- register-search-tool.ts
    |-- register-extract-tool.ts
    |-- register-crawl-tool.ts
    `-- register-research-tools.ts
```

- `src/index.ts`: creates the server and connects the STDIO transport. Diagnostics go to `stderr` because `stdout` is reserved for MCP messages.
- `src/server.ts`: defines the server identity and cross-tool instructions, then registers every public tool.
- `src/config.ts`: reads and validates `TAVILY_API_KEY` without logging it.
- `src/clients/tavily-http-client.ts`: centralizes the Tavily base URL, Bearer authentication, JSON encoding and parsing, configurable timeouts, and useful HTTP errors.
- `src/clients/tavily-content-options.ts`: contains content formats, extraction depths, shared TypeScript types, and image normalization used by Extract and Crawl.
- `src/clients/tavily-search-client.ts`: builds Search requests, validates Search responses, and normalizes ranked results.
- `src/clients/tavily-extract-client.ts`: sends one Extract request for up to 20 known URLs, keeps partial failures, and normalizes page content and metadata.
- `src/clients/tavily-crawl-client.ts`: sends a controlled Crawl request with depth, breadth, result limits, regex filters, and extraction options.
- `src/clients/tavily-research-client.ts`: creates asynchronous Research tasks, retrieves task status, validates responses, and normalizes reports and sources.
- `src/tools/register-search-tool.ts`: defines the `tavily_search` MCP contract and formats its output.
- `src/tools/register-extract-tool.ts`: defines the `tavily_extract` MCP contract and reports successful and failed URLs separately.
- `src/tools/register-crawl-tool.ts`: defines the `tavily_crawl` MCP contract and applies conservative defaults for scope and cost.
- `src/tools/register-research-tools.ts`: defines `tavily_research` and `tavily_research_status`, including the required create-then-poll workflow.

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

Save the server and select **Restart** in the MCP server settings. The following tools should then appear:

```text
tavily_search
tavily_extract
tavily_crawl
tavily_research
tavily_research_status
```

Codex uses a 60-second tool timeout by default. If you intentionally configure a longer Crawl timeout, raise `tool_timeout_sec` for this server as described in the [official Codex MCP documentation](https://developers.openai.com/codex/mcp/).

## Test with MCP Inspector

Build the project, then launch the Inspector from the repository root:

```powershell
npm run build
npx @modelcontextprotocol/inspector node --env-file-if-exists=.env dist/index.js
```

Start with a small `tavily_search` or one-URL `tavily_extract` request. Avoid using Research or a broad Crawl merely as a connection test because they can consume more credits.

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

### Extract known pages

```json
{
  "urls": [
    "https://nodejs.org/en/about/previous-releases"
  ],
  "query": "Current and maintenance LTS release lines",
  "chunksPerSource": 3,
  "extractDepth": "basic",
  "format": "markdown"
}
```

`query` is optional. When it is supplied, Tavily reranks page content and `chunksPerSource` controls how many relevant chunks are returned for each URL.

### Crawl a website

```json
{
  "url": "https://docs.tavily.com",
  "instructions": "Find pages documenting Search, Extract, Crawl, and Research endpoints.",
  "maxDepth": 1,
  "maxBreadth": 10,
  "limit": 10,
  "selectPaths": ["/documentation/.*"],
  "allowExternal": false,
  "extractDepth": "basic",
  "format": "markdown",
  "timeout": 30
}
```

This server caps `limit` at 50 pages even though Tavily may support larger requests. The cap controls response size and accidental credit usage. Natural-language `instructions` are useful but increase the mapping portion of Crawl cost.

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

Tavily currently documents the following credit model:

- Search with `basic`: 1 credit per request.
- Search with `advanced`: 2 credits per request.
- Extract with `basic`: 1 credit for every 5 successful URL extractions.
- Extract with `advanced`: 2 credits for every 5 successful URL extractions.
- Failed extractions are not charged.
- Crawl combines mapping and extraction costs. Crawling 10 pages costs approximately 3 credits with basic extraction or 5 with advanced extraction; guided mapping with `instructions` increases the mapping portion.
- Research with `mini`: dynamically between 4 and 110 credits.
- Research with `pro`: dynamically between 15 and 250 credits.

Pricing and limits can change. Review the [official Tavily credits documentation](https://docs.tavily.com/documentation/api-credits) before running large or repeated tasks.

## Safety defaults

- Search defaults to `basic` depth.
- Extract defaults to `basic`, Markdown, no images, and no favicon.
- Crawl defaults to depth `1`, breadth `10`, limit `10`, `basic` extraction, a 30-second timeout, and no external-domain results.
- Crawl has a project-level maximum of 50 returned pages.
- Research is never selected as the default workflow because it creates a persistent billable task.
- Every endpoint requests usage metadata when supported so the response can report consumed credits.

## Security notes

- Keep the API key only in `.env` or another private environment source.
- Never print credentials to `stdout` or `stderr`.
- Never write ordinary logs to `stdout`; STDIO MCP reserves it for protocol messages.
- Treat web content returned by Tavily as untrusted external input.
- Review generated research and verify important claims against primary sources.

## Scripts

| Command | Description |
| --- | --- |
| `npm run clean` | Delete generated files from `dist/`. Source files are not touched. |
| `npm run dev` | Run `src/index.ts` directly with `tsx` and load `.env` when it exists. |
| `npm run typecheck` | Ask TypeScript to validate types without creating output files. |
| `npm run build` | Clean `dist/`, then compile TypeScript into JavaScript, declarations, and source maps. |
| `npm run check` | Run type checking first and then perform a clean production build. |
| `npm start` | Run the compiled `dist/index.js` server and load `.env` when it exists. |

## Roadmap

- Add automated tests for API validation and MCP output formatting.
- Add the Tavily Map endpoint if a discovery-only site map workflow is needed.
- Add explicit MCP output schemas for structured responses.
- Prepare the package for npm distribution after the public API stabilizes.

## License

This project is licensed under the ISC License.
