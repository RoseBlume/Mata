param([ValidateSet('cuda','hip')][string]$Runtime)
$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    if ($Runtime -eq 'cuda') {
        & nvcc -O3 -std=c++17 -shared -Xcompiler '/MD' -o mataimage_cuda.dll mataimage_gpu.cu
    } else {
        & hipcc -O3 -std=c++17 -DMATA_HIP -x hip -shared -o mataimage_hip.dll mataimage_gpu.cu
    }
    if ($LASTEXITCODE -ne 0) { throw "GPU library build failed ($LASTEXITCODE)" }
} finally { Pop-Location }
