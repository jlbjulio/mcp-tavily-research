# MCP Tavily Research

An MCP server that gives coding agents Tavily Search, Extract, Crawl, and Research, as a single small executable.

- About 2 MB on disk and under 10 MB of RAM per agent session, with no Node.js or `node_modules`.
- Starts in about 10 ms.
- Works with Claude Code, Codex, and any MCP client that launches stdio servers.

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

Requirements: [Rust](https://rustup.rs/) and a [Tavily API key](https://app.tavily.com/).

```powershell
git clone https://github.com/jlbjulio/mcp-tavily-research.git
cd mcp-tavily-research
cargo install --path .
Copy-Item .env.example .env
```

Add your API key to `.env`:

```dotenv
TAVILY_API_KEY=your_real_tavily_api_key
```

`cargo install` puts `mcp-tavily-research` in Cargo's bin folder (`~/.cargo/bin`), which is on your `PATH`. The `.env` file is ignored by Git. Never commit your API key.

## Connect to Claude Code

```powershell
claude mcp add tavily-research --scope user -- mcp-tavily-research --env-file-if-exists=C:/path/to/mcp-tavily-research/.env
```

## Connect to Codex

Create a local STDIO MCP server with these values:

| Field | Value |
| --- | --- |
| Name | `tavily-research` |
| Command to launch | `mcp-tavily-research` |
| Arguments | `--env-file-if-exists=.env` |
| Environment variables | Leave empty |
| Environment variable passthrough | Leave empty |
| Working directory | Absolute path to this repository |

Save the configuration and restart the MCP server. You should see:

```text
tavily_search
tavily_extract
tavily_crawl
tavily_research
tavily_research_status
```

See the [Codex MCP documentation](https://developers.openai.com/codex/mcp/) for manual configuration and timeout options.

Instead of an `.env` file you can also set `TAVILY_API_KEY` in the client's environment variables.

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
| `cargo install --path .` | Build and install the server. Close the MCP clients using it first. |
| `cargo test` | Run the test suite. |
| `cargo build --release` | Build `target/release/mcp-tavily-research.exe` without installing it. |

## Test with MCP Inspector

```powershell
npx @modelcontextprotocol/inspector mcp-tavily-research --env-file-if-exists=.env
```

Use Search or a one-URL Extract request for the first test to keep credit usage low.

## License

ISC
