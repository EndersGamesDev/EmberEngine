# Arena v13 meshing runbook: concept view PNGs -> GLBs via Hunyuan3D-2mv
# (C:\hy3d). Pauses the local GLM worker for VRAM and restarts it even on
# failure. Idempotent: existing outputs are skipped, so a re-run resumes.
#
# Same shape as the Fire Racer runbook (.claude/fire-castle/mesh-props.ps1),
# checked in this time so the next map does not start from a chat transcript.
# Differences: the "v13-" prefix, the views live in assets/concepts/v13/ where
# tools/v13/fetch_pictures.py put them, and the second decimation budget is
# per part: the skyline pieces are seen from 40 m and get 4000 faces, the
# statue and sandbags are stared at from arm's length and keep 8000.
#
#   pwsh -File tools\v13\mesh-props.ps1               # every prop
#   pwsh -File tools\v13\mesh-props.ps1 -Parts statue # one
param(
    [string]$ViewsDir = 'C:\Users\end\dev\ember\assets\concepts\v13',
    [string]$OutDir = 'C:\Users\end\dev\ember\assets\models\v13',
    [string[]]$Parts = @('statue', 'sandbags', 'cathedral', 'facade-a', 'facade-b', 'wreck', 'lamp')
)
$ErrorActionPreference = 'Continue'
$startTime = Get-Date
$ok = 0; $skipped = 0; $failed = 0
$budget = @{ statue = 8000; sandbags = 8000; cathedral = 6000; 'facade-a' = 4000; 'facade-b' = 4000; wreck = 5000; lamp = 3000 }

taskkill /im llama-server.exe /f 2>$null | Out-Null
Start-Sleep -Seconds 3
New-Item -ItemType Directory -Force $OutDir | Out-Null

try {
    $i = 0
    foreach ($part in $Parts) {
        $i++
        $t0 = Get-Date
        $name = "v13-$part"
        $out = Join-Path $OutDir "$part.glb"
        if (Test-Path $out) {
            Write-Output "[mesh] $i/$($Parts.Count) $part skipped (output exists)"
            $skipped++
            continue
        }
        # Collect views in the order gen3d_mv.py expects; front is required.
        $found = @()
        $labels = @()
        foreach ($view in 'front', 'back', 'left', 'right') {
            $f = Join-Path $ViewsDir "$name-$view.png"
            if (Test-Path $f) { $found += $f; $labels += $view }
            elseif ($view -eq 'front') { break }
        }
        if ($labels.Count -eq 0 -or $labels[0] -ne 'front') {
            Write-Output "[mesh] $i/$($Parts.Count) $part skipped (no front view in $ViewsDir)"
            $skipped++
            continue
        }
        & C:\hy3d\venv\Scripts\python.exe C:\hy3d\gen3d_mv.py $out @found
        if ($LASTEXITCODE -eq 0 -and (Test-Path $out)) {
            $raw = [math]::Round((Get-Item $out).Length / 1MB, 2)
            # Second pass: gen3d_mv leaves 30k faces. Every triangle here is
            # bytes in the bundle, so cut hard and measure the result.
            $target = $budget[$part]
            if (-not $target) { $target = 6000 }
            & C:\hy3d\venv\Scripts\python.exe C:\hy3d\decimate.py $target $out --no-backup
            $mb = [math]::Round((Get-Item $out).Length / 1MB, 2)
            $s = [math]::Round(((Get-Date) - $t0).TotalSeconds)
            Write-Output "[mesh] $i/$($Parts.Count) $part ($($labels -join ',')) -> $part.glb ($raw MB -> $mb MB, $target faces, ${s}s)"
            $ok++
        } else {
            Write-Output "[mesh] $i/$($Parts.Count) $part FAILED (exit $LASTEXITCODE)"
            $failed++
        }
    }
} finally {
    # Restart the worker whatever happened above — leaving the user's GLM
    # server dead because a mesh failed is not an acceptable outcome.
    Start-Process -WindowStyle Hidden cmd -ArgumentList '/c', 'C:\llama.cpp\start-glm-server.cmd'
}
$min = [math]::Round(((Get-Date) - $startTime).TotalMinutes, 1)
Write-Output "[mesh] SUMMARY: $ok ok, $skipped skipped, $failed failed, $min min"
