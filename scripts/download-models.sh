#!/usr/bin/env bash
# Downloads the llama-server binary (Vulkan build - works on NVIDIA/AMD/Intel GPUs
# without needing the CUDA toolkit) and the Qwen3-8B GGUF model used by llmlocal.
set -euo pipefail

LLAMA_RELEASE="b9873"
BIN_DIR="$HOME/.cache/llmlocal/bin"
MODEL_DIR="$HOME/.cache/llmlocal/models"

mkdir -p "$BIN_DIR" "$MODEL_DIR"

if [ ! -x "$BIN_DIR/llama-server" ]; then
  echo "Downloading llama.cpp ${LLAMA_RELEASE} (Vulkan, Ubuntu x64)..."
  tmp=$(mktemp -d)
  curl -L -o "$tmp/llama-vulkan.tar.gz" \
    "https://github.com/ggml-org/llama.cpp/releases/download/${LLAMA_RELEASE}/llama-${LLAMA_RELEASE}-bin-ubuntu-vulkan-x64.tar.gz"
  tar xzf "$tmp/llama-vulkan.tar.gz" -C "$tmp"
  cp -r "$tmp"/llama-*/* "$BIN_DIR/"
  rm -rf "$tmp"
  echo "llama-server installed at $BIN_DIR/llama-server"
else
  echo "llama-server already present at $BIN_DIR/llama-server"
fi

if [ ! -f "$MODEL_DIR/Qwen3-8B-Q4_K_M.gguf" ]; then
  echo "Downloading Qwen3-8B-Q4_K_M.gguf (~4.7GB)..."
  curl -L -o "$MODEL_DIR/Qwen3-8B-Q4_K_M.gguf" \
    "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/Qwen3-8B-Q4_K_M.gguf"
else
  echo "Model already present at $MODEL_DIR/Qwen3-8B-Q4_K_M.gguf"
fi

echo "Done."
