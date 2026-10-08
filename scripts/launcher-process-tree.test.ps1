$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/launcher-process-tree.ps1"
$taskSnapshot = @(
    [pscustomobject]@{ ProcessId = 4; ParentProcessId = 3; Name = 'msedgewebview2.exe' },
    [pscustomobject]@{ ProcessId = 3; ParentProcessId = 1; Name = 'msedgewebview2.exe' },
    [pscustomobject]@{ ProcessId = 5; ParentProcessId = 1; Name = 'javaw.exe' },
    [pscustomobject]@{ ProcessId = 6; ParentProcessId = 5; Name = 'msedgewebview2.exe' },
    [pscustomobject]@{ ProcessId = 7; ParentProcessId = 99; Name = 'msedgewebview2.exe' }
)
$taskFound = @(Get-LauncherProcessIds -RootId 1 -Snapshot $taskSnapshot)
if (($taskFound -join ',') -ne '1,3,4') { throw "Wrong process selection: $taskFound" }
Write-Output 'Process tree fixture passed: nested WebView2 only; unrelated browsers and Minecraft excluded.'
