#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")"
case "${1:-}" in
  cuda) nvcc -O3 -std=c++17 -shared -Xcompiler -fPIC mataimage_gpu.cu -o libmataimage_cuda.so ;;
  hip) hipcc -O3 -std=c++17 -DMATA_HIP -x hip -shared -fPIC mataimage_gpu.cu -o libmataimage_hip.so ;;
  *) echo 'Usage: ./build.sh cuda|hip' >&2; exit 2 ;;
esac
