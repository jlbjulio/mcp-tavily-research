# MCP Tavily Research

A TypeScript MCP server that gives coding agents access to Tavily Search, Extract, Crawl, and Research.

> Personal project. Not affiliated with Tavily.

## Tools

| Tool | Use it for |
| --- | --- |
| `tavily_search` | Find current information and sources. |
| `tavily_extract` | Read content from known URLs. |
| `tavily_crawl` | Read multiple pages from one website. |
| `tavily_research` | Start a deep, multi-source investigation. |
| `tavily_research_status` | Get the status or result of a Research task. |

Start with `tavily_search` for most questions. Research is slower and can use considerably more credits.

Tool parameters use Tavily's original `snake_case` names, such as `search_depth`, `max_results`, and `request_id`.

## Quick start

Requirements: Node.js 22+, npm, and a [Tavily API key](https://app.tavily.com/).

```powershell
git clone https://github.com/jlbjulio/mcp-tavily-research.git
cd mcp-tavily-research
npm install
Copy-Item .env.example .env
```

Add your API key to `.env`:

```dotenv
TAVILY_API_KEY=your_real_tavily_api_key
```

Build the server:

```powershell
npm run check
```

The `.env` file is ignored by Git. Never commit your API key.

## Connect to Codex

Create a local STDIO MCP server with these values:

| Field | Value |
| --- | --- |
| Name | `tavily-research` |
| Command to launch | `node` |
| Arguments | `--env-file-if-exists=.env` and `dist/index.js` |
| Environment variables | Leave empty |
| Environment variable passthrough | Leave empty |
| Working directory | Absolute path to this repository |

Example working directory:

```text
C:\Users\your-name\Documents\GitHub\mcp-tavily-research
```

Save the configuration and restart the MCP server. You should see:

```text
tavily_search
tavily_extract
tavily_crawl
tavily_research
tavily_research_status
```

See the [Codex MCP documentation](https://developers.openai.com/codex/mcp/) for manual configuration and timeout options.

## Example prompts

- `Use tavily_search to find the current Node.js LTS version. Use basic search and return three official sources.`
- `Use tavily_extract to read these URLs and summarize their differences: <urls>.`
- `Use tavily_crawl on https://docs.example.com. Only inspect /api/ pages, use depth 1, and return at most 10 pages.`
- `Use tavily_research with the mini model to compare Node.js 22 and Node.js 24 using official sources.`

Research is asynchronous. Use the returned request ID with `tavily_research_status`; do not start the same task again.

## Credit safety

- Search and Extract use `basic` depth by default.
- Extract accepts up to 20 URLs.
- Crawl defaults to depth 1 and a limit of 10 pages.
- External domains are disabled in Crawl by default.
- This server caps Crawl results at 50 pages.
- Research uses the lower-cost `mini` model by default.

Check [Tavily's current credit documentation](https://docs.tavily.com/documentation/api-credits) before running broad crawls or repeated Research tasks.

## Commands

| Command | Description |
| --- | --- |
| `npm run dev` | Run the TypeScript source with `tsx`. |
| `npm run typecheck` | Check TypeScript without creating files. |
| `npm run build` | Compile the project into `dist/`. |
| `npm run check` | Run type checking and a clean build. |
| `npm run clean` | Delete generated files from `dist/`. |
| `npm start` | Run the compiled MCP server. |

## Project structure

- `src/tools/`: tools exposed to MCP clients.
- `src/tavily/api.ts`: shared connection, authentication, errors, and response validation.
- `src/tavily/search.ts`, `extract.ts`, `crawl.ts`, and `research.ts`: Tavily endpoint logic.
- `src/server.ts`: server setup and tool registration.
- `src/index.ts`: STDIO entry point.
- `src/config.ts`: API key configuration.

## Test with MCP Inspector

```powershell
npm run build
npx @modelcontextprotocol/inspector node --env-file-if-exists=.env dist/index.js
```

Use Search or a one-URL Extract request for the first test to keep credit usage low.

## License

ISC
