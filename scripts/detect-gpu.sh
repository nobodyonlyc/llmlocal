#!/usr/bin/env bash
# Detects whether an NVIDIA GPU is usable from containers (driver present +
# nvidia-container-toolkit's CDI spec generated). Prints "gpu" or "cpu" on
# stdout; used by scripts/dev-up.sh to pick the compose override.
set -euo pipefail

has_driver() {
  command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi -L >/dev/null 2>&1
}

has_cdi_spec() {
  [ -f /etc/cdi/nvidia.yaml ] || [ -f /var/run/cdi/nvidia.yaml ]
}

if has_driver && has_cdi_spec; then
  echo "gpu"
else
  echo "cpu"
fi
