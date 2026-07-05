#!/usr/bin/env bash
# Starts the local dev stack: llama-server (native, GPU via Vulkan) in the
# background, and Qdrant via podman-compose.
set -euo pipefail

BIN_DIR="$HOME/.cache/llmlocal/bin"
MODEL_DIR="$HOME/.cache/llmlocal/models"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

podman-compose -f "$REPO_DIR/deploy/podman-compose.yml" up -d

"$BIN_DIR/llama-server" \
  -m "$MODEL_DIR/Qwen3-8B-Q4_K_M.gguf" \
  --host 127.0.0.1 --port 8080 \
  -ngl 999 -c 8192 --parallel 2 -cb --jinja \
  > /tmp/llama-server.log 2>&1 &

echo "llama-server starting in background (PID $!), logs at /tmp/llama-server.log"
echo "Qdrant: http://127.0.0.1:6333"
echo "llama-server: http://127.0.0.1:8080 (wait a few seconds for model load)"
