param([switch]$Release)

$ErrorActionPreference = 'Stop'

# BoltFFI links its generated C bridge with cl.exe. Load the installed MSVC
# environment so mise works from an ordinary PowerShell terminal.
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) {
        throw 'Install Visual Studio Build Tools with Desktop development with C++ and a Windows SDK.'
    }
    $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $installation) { throw 'The Visual Studio C++ build tools are missing.' }
    Import-Module (Join-Path $installation 'Common7/Tools/Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64'
}

Push-Location (Join-Path $PSScriptRoot '../../bindings/ffi')
try {
    $buildArguments = @('pack', 'csharp', '--deny-skipped')
    if ($Release) { $buildArguments += '--release' }
    & boltffi @buildArguments
    if ($LASTEXITCODE -ne 0) { throw "C# binding generation failed with exit code $LASTEXITCODE." }
} finally { Pop-Location }
