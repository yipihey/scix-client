#!/bin/sh
# Build the Claude Desktop extension bundle (.mcpb) for scix.
#
# Usage:
#   scripts/build-mcpb.sh VERSION DARWIN_BINARY WINDOWS_EXE OUTPUT.mcpb
#
# DARWIN_BINARY should be a universal (x86_64 + arm64) Mach-O binary; make one
# with `lipo -create -output scix scix-x86_64 scix-aarch64`.
#
# An .mcpb is a zip with manifest.json at the root. The tools list in the
# manifest is read from the binary itself so it never drifts from the server.
# Packs with the official `mcpb` CLI when npx is available, falling back to zip.
set -eu

VERSION="$1"
DARWIN_BIN="$2"
WINDOWS_EXE="$3"
OUT="$4"

HERE=$(cd "$(dirname "$0")" && pwd)
STAGE=$(mktemp -d 2>/dev/null || mktemp -d -t scix-mcpb)
trap 'rm -rf "$STAGE"' EXIT INT TERM

mkdir -p "$STAGE/server"
cp "$DARWIN_BIN" "$STAGE/server/scix"
cp "$WINDOWS_EXE" "$STAGE/server/scix.exe"
chmod 755 "$STAGE/server/scix"
cp "$HERE/../LICENSE-MIT" "$STAGE/LICENSE"

# Ask the server for its tool list (works without a token).
TOOLS_JSON=$(printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"build-mcpb","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | SCIX_API_TOKEN= "$STAGE/server/scix" serve | tail -n 1)

printf '%s' "$TOOLS_JSON" | VERSION="$VERSION" python3 -I -c '
import json, os, sys
tools = [{"name": t["name"], "description": t["description"]}
         for t in json.load(sys.stdin)["result"]["tools"]]
manifest = {
  "manifest_version": "0.3",
  "name": "scix",
  "display_name": "SciX / NASA ADS",
  "version": os.environ["VERSION"],
  "description": "Search the astronomy and physics literature (SciX / NASA ADS): papers, citations, metrics, BibTeX export, and your ADS libraries.",
  "long_description": "Gives Claude direct access to the SciX / NASA ADS database: full-text literature search with field queries, single-paper details, citation and reference networks, h-index and other metrics, export in BibTeX/AASTeX/RIS and more, object name resolution via SIMBAD/NED, and management of your personal ADS libraries.\n\nYou need a free API token from https://ui.adsabs.harvard.edu/user/settings/token",
  "author": {"name": "Tom Abel", "url": "https://github.com/yipihey"},
  "repository": {"type": "git", "url": "https://github.com/yipihey/scix-client"},
  "homepage": "https://github.com/yipihey/scix-client",
  "documentation": "https://github.com/yipihey/scix-client/blob/main/docs/mcp-server.md",
  "support": "https://github.com/yipihey/scix-client/issues",
  "license": "MIT",
  "keywords": ["astronomy", "astrophysics", "ads", "scix", "literature", "bibliography", "citations"],
  "server": {
    "type": "binary",
    "entry_point": "server/scix",
    "mcp_config": {
      "command": "${__dirname}/server/scix",
      "args": ["serve"],
      "env": {"SCIX_API_TOKEN": "${user_config.api_token}"},
      "platform_overrides": {
        "win32": {"command": "${__dirname}/server/scix.exe"}
      }
    }
  },
  "tools": tools,
  "tools_generated": False,
  "user_config": {
    "api_token": {
      "type": "string",
      "title": "SciX / ADS API token",
      "description": "Free token from https://ui.adsabs.harvard.edu/user/settings/token (sign in, then Settings > API Token).",
      "sensitive": True,
      "required": True
    }
  },
  "compatibility": {
    "claude_desktop": ">=0.10.0",
    "platforms": ["darwin", "win32"]
  }
}
json.dump(manifest, open(os.path.join(sys.argv[1], "manifest.json"), "w"), indent=2)
print("manifest.json: %d tools" % len(tools))
' "$STAGE"

OUT_ABS=$(cd "$(dirname "$OUT")" && pwd)/$(basename "$OUT")
rm -f "$OUT_ABS"
if command -v npx >/dev/null 2>&1 && npx -y @anthropic-ai/mcpb pack "$STAGE" "$OUT_ABS"; then
  :
else
  echo "mcpb CLI unavailable; packing with zip" >&2
  (cd "$STAGE" && zip -q -r "$OUT_ABS" manifest.json server LICENSE)
fi
echo "wrote $OUT_ABS"
