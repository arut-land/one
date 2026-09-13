[CmdletBinding(SupportsShouldProcess = $true)]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$repoPrefix = $repoRoot.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar

function Assert-WorkspacePath([string]$Path) {
    $absolute = [IO.Path]::GetFullPath($Path)
    if (-not $absolute.StartsWith($repoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Cleanup target is outside the repository: $absolute"
    }
    # Reject redirected targets or parents before any recursive deletion.
    $parent = $absolute
    while ($parent.Length -ge $repoRoot.Length) {
        if (Test-Path -LiteralPath $parent) {
            $item = Get-Item -LiteralPath $parent -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Cleanup target traverses a reparse point: $parent"
            }
        }
        $parent = [IO.Path]::GetDirectoryName($parent)
    }
    return $absolute
}

Push-Location -LiteralPath $repoRoot
try {
    $metadata = & cargo metadata --no-deps --format-version 1 --offline
    if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the Cargo cleanup directory.' }
    $cargoTarget = Assert-WorkspacePath (($metadata | ConvertFrom-Json).target_directory)
    $targets = @(
        Join-Path $repoRoot 'bindings/generated'
        Join-Path $repoRoot 'node_modules'
        Join-Path $repoRoot 'bindings/typescript/node_modules'
        foreach ($surface in Get-ChildItem -LiteralPath (Join-Path $repoRoot 'surfaces') -Directory) {
            Join-Path $surface.FullName 'dist'
            Join-Path $surface.FullName 'node_modules'
        }
    ) | ForEach-Object { Assert-WorkspacePath $_ }

    if ($PSCmdlet.ShouldProcess($cargoTarget, 'cargo clean')) {
        & cargo clean
        if ($LASTEXITCODE -ne 0) { throw 'cargo clean failed.' }
    }
    foreach ($target in $targets) {
        if ((Test-Path -LiteralPath $target) -and $PSCmdlet.ShouldProcess($target, 'Remove build output')) {
            # Directory.Delete removes nested junctions/symlinks themselves; it
            # does not traverse their targets, including pnpm workspace links.
            [IO.Directory]::Delete($target, $true)
        }
    }
}
finally {
    Pop-Location
}
