#!/usr/bin/env bash
# Brings up the full llmlocal stack in containers: Qdrant, llama-server
# (GPU/CUDA if available, CPU otherwise), and the API server. Nothing runs
# natively on the host.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_BASE="$REPO_DIR/deploy/podman-compose.yml"
COMPOSE_GPU="$REPO_DIR/deploy/podman-compose.gpu.yml"

# So SERVER_PORT (and other overrides) from .env are visible below, matching
# what --env-file passes into the compose services.
ENV_FILE_ARGS=()
if [ -f "$REPO_DIR/.env" ]; then
  set -a
  source "$REPO_DIR/.env"
  set +a
  ENV_FILE_ARGS=(--env-file "$REPO_DIR/.env")
fi

if command -v podman-compose >/dev/null 2>&1; then
  COMPOSE=(podman-compose)
elif command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  COMPOSE=(docker compose)
else
  echo "error: need podman-compose or 'docker compose' on PATH" >&2
  exit 1
fi

MODE="$("$REPO_DIR/scripts/detect-gpu.sh")"

if [ "$MODE" = "gpu" ]; then
  echo "GPU detected (nvidia-smi + CDI spec present) -> running llama-server on CUDA"
  "${COMPOSE[@]}" "${ENV_FILE_ARGS[@]}" -f "$COMPOSE_BASE" -f "$COMPOSE_GPU" up -d --build
else
  echo "No usable GPU found (missing driver or nvidia-container-toolkit CDI spec) -> running llama-server on CPU"
  echo "See README.md for how to enable GPU passthrough."
  "${COMPOSE[@]}" "${ENV_FILE_ARGS[@]}" -f "$COMPOSE_BASE" up -d --build
fi

echo
echo "Qdrant:       http://127.0.0.1:6333"
echo "llama-server: http://127.0.0.1:8080 (wait for model load on first run)"
echo "API:          http://127.0.0.1:${SERVER_PORT:-3000}"
