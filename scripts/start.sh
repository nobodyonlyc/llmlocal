#!/usr/bin/env bash
# Resumes the llmlocal stack after a reboot: starts existing containers if
# present, or creates them (GPU/CPU auto-detected, same as dev-up.sh) if
# this is a fresh environment. Skips --build for a fast start; use
# dev-up.sh instead after changing source code or the Dockerfile.
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
  echo "GPU detected -> starting/creating llama-server on CUDA"
  "${COMPOSE[@]}" "${ENV_FILE_ARGS[@]}" -f "$COMPOSE_BASE" -f "$COMPOSE_GPU" up -d
else
  echo "No usable GPU -> starting/creating llama-server on CPU"
  "${COMPOSE[@]}" "${ENV_FILE_ARGS[@]}" -f "$COMPOSE_BASE" up -d
fi

API_PORT="${SERVER_PORT:-3000}"
echo
echo "Waiting for API to become ready on port ${API_PORT}..."
for i in $(seq 1 60); do
  if curl -sf "http://127.0.0.1:${API_PORT}/readyz" >/dev/null 2>&1; then
    echo "API ready:"
    curl -s "http://127.0.0.1:${API_PORT}/readyz"; echo
    exit 0
  fi
  sleep 1
done

echo "warning: API did not report ready within 60s — check 'podman logs deploy_api_1'" >&2
exit 1
