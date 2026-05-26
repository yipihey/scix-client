# Chat REPL Guide

[Back to README](../README.md)

`scix chat` is a built-in tool-use REPL that drives any tool-use-capable LLM
against the SciX tool set — no MCP host required. Set one provider's API key
and a SciX API token, and go.

## Quick Start

```bash
export SCIX_API_TOKEN="your-ads-token"
export ANTHROPIC_API_KEY="sk-ant-..."

scix chat
```

Inside the REPL you'll see a `>` prompt. Type a question; press Enter. The
model has access to the full SciX tool set; tool calls and results are
printed inline as they happen.

```
> Find the three most-cited papers by Vera Rubin on galaxy rotation curves
  [tool] scix_search({"query":"author:\"Rubin, V\" title:\"rotation curve\"","sort":"citation_count desc","rows":3})

The three most-cited papers by Vera Rubin on galaxy rotation curves are:
  1. ...
> exit
```

## Providers

Provider is selected with `--provider`, or auto-detected from the first env
var present in this order: `ANTHROPIC_API_KEY` → `OPENAI_API_KEY` →
`GEMINI_API_KEY` → `OLLAMA_HOST`.

| Provider     | Env var                        | Default model         | Notes |
|--------------|--------------------------------|-----------------------|-------|
| `anthropic`  | `ANTHROPIC_API_KEY`            | `claude-sonnet-4-6`   |       |
| `openai`     | `OPENAI_API_KEY`               | `gpt-5`               | Optional `OPENAI_BASE_URL` for compatible APIs |
| `gemini`     | `GEMINI_API_KEY` / `GOOGLE_API_KEY` | `gemini-2.5-pro` |       |
| `ollama`     | `OLLAMA_HOST` (default `http://localhost:11434`) | `llama3.1:8b` | Uses Ollama's OpenAI-compatible `/v1` endpoint |

Override the model with `--model`:

```bash
scix chat --provider openai --model gpt-5
scix chat --provider gemini --model gemini-2.5-flash
scix chat --provider ollama --model llama3.1:70b
```

## Options

```
scix chat [OPTIONS]

  --provider <P>      anthropic | openai | gemini | ollama
  --model <NAME>      Override the default model
  --max-turns <N>     Maximum tool-call iterations per user message (default 25)
```

## How It Compares to MCP

| Aspect            | `scix serve` (MCP)             | `scix chat`                       |
|-------------------|--------------------------------|-----------------------------------|
| Host required     | Yes (Claude Code, Cursor, ...) | No — direct API call              |
| Bring your own key | Host's key                    | Yours (one of four providers)     |
| Streaming         | Host-dependent                 | Not yet                           |
| Best for          | Daily use in an IDE/editor     | Scripting, CI, headless servers, models without an MCP host |

Both modes share the same tool implementations, so behavior is identical.

## Troubleshooting

- **"No LLM provider detected"** — set one of `ANTHROPIC_API_KEY`,
  `OPENAI_API_KEY`, `GEMINI_API_KEY`, or `OLLAMA_HOST`, or pass `--provider`.
- **"401 unauthorized"** — your provider key is missing or wrong.
- **"Authentication required"** — your SciX API token is missing
  (`SCIX_API_TOKEN`).
- **Tool calls failing repeatedly** — increase `--max-turns`, or run
  `scix doctor` to verify connectivity.
