param(
    [Parameter(Mandatory = $true)] [int] $ProcessId,
    [ValidateRange(2, 60)] [int] $Seconds = 10
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/launcher-process-tree.ps1"
$taskProcess = Get-Process -Id $ProcessId
$taskRootStarted = $taskProcess.StartTime
$taskStartCpu = $taskProcess.TotalProcessorTime.TotalMilliseconds
$taskTimer = [Diagnostics.Stopwatch]::StartNew()
$taskSamples = [Collections.Generic.List[object]]::new()
$taskCpuStarts = @{}
$taskCpuEnds = @{}
$taskIncomplete = $false
for ($taskIndex = 0; $taskIndex -lt $Seconds; $taskIndex++) {
    $taskProcess.Refresh()
    if ($taskProcess.HasExited -or $taskProcess.StartTime -ne $taskRootStarted) { break }
    $taskTree = @($ProcessId)
    try {
        $taskTree = @(Get-LauncherProcessIds -RootId $ProcessId -Snapshot @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name))
    } catch { $taskIncomplete = $true }
    $taskSet = 0L
    $taskPrivate = 0L
    $taskHandles = 0L
    $taskCurrentIdentities = [Collections.Generic.HashSet[string]]::new()
    foreach ($taskMemberId in $taskTree) {
        try {
            $taskMember = Get-Process -Id $taskMemberId -ErrorAction Stop
            $taskIdentity = "$taskMemberId/$($taskMember.StartTime.ToUniversalTime().Ticks)"
            $null = $taskCurrentIdentities.Add($taskIdentity)
            $taskCpu = $taskMember.TotalProcessorTime.TotalMilliseconds
            if (-not $taskCpuStarts.ContainsKey($taskIdentity)) {
                $taskCpuStarts[$taskIdentity] = $taskCpu
                if ($taskIndex -gt 0) { $taskIncomplete = $true }
            }
            $taskCpuEnds[$taskIdentity] = $taskCpu
            $taskSet += $taskMember.WorkingSet64
            $taskPrivate += $taskMember.PrivateMemorySize64
            $taskHandles += $taskMember.HandleCount
        } catch { $taskIncomplete = $true }
    }
    foreach ($taskIdentity in $taskCpuStarts.Keys) {
        if (-not $taskCurrentIdentities.Contains($taskIdentity)) { $taskIncomplete = $true }
    }
    $taskSamples.Add([pscustomobject]@{
        elapsedMilliseconds = $taskTimer.ElapsedMilliseconds
        workingSetBytes = $taskProcess.WorkingSet64
        privateMemoryBytes = $taskProcess.PrivateMemorySize64
        handles = $taskProcess.HandleCount
        processTreeIds = $taskTree
        treeWorkingSetBytes = $taskSet
        treePrivateMemoryBytes = $taskPrivate
        treeHandles = $taskHandles
    })
    Start-Sleep -Milliseconds 1000
}
# Capture the endpoint after the last sampling interval, just as for root CPU.
# Otherwise tree CPU would omit the final second while using the full duration.
try {
    $taskFinalTree = @(Get-LauncherProcessIds -RootId $ProcessId -Snapshot @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name))
    $taskFinalIdentities = [Collections.Generic.HashSet[string]]::new()
    foreach ($taskMemberId in $taskFinalTree) {
        try {
            $taskMember = Get-Process -Id $taskMemberId -ErrorAction Stop
            $taskIdentity = "$taskMemberId/$($taskMember.StartTime.ToUniversalTime().Ticks)"
            $null = $taskFinalIdentities.Add($taskIdentity)
            $taskCpu = $taskMember.TotalProcessorTime.TotalMilliseconds
            if (-not $taskCpuStarts.ContainsKey($taskIdentity)) {
                $taskCpuStarts[$taskIdentity] = $taskCpu
                $taskIncomplete = $true
            }
            $taskCpuEnds[$taskIdentity] = $taskCpu
        } catch { $taskIncomplete = $true }
    }
    foreach ($taskIdentity in $taskCpuStarts.Keys) {
        if (-not $taskFinalIdentities.Contains($taskIdentity)) { $taskIncomplete = $true }
    }
} catch { $taskIncomplete = $true }
$taskProcess.Refresh()
$taskCpuMilliseconds = $taskProcess.TotalProcessorTime.TotalMilliseconds - $taskStartCpu
$taskDurationMilliseconds = $taskTimer.ElapsedMilliseconds
$taskTreeCpuMilliseconds = 0.0
foreach ($taskIdentity in $taskCpuStarts.Keys) {
    $taskTreeCpuMilliseconds += [Math]::Max(0, $taskCpuEnds[$taskIdentity] - $taskCpuStarts[$taskIdentity])
}
[pscustomobject]@{
    processId = $ProcessId
    durationMilliseconds = $taskDurationMilliseconds
    cpuMilliseconds = $taskCpuMilliseconds
    # Percentage of one logical CPU; report core count to interpret it.
    cpuPercentOfOneCore = [Math]::Round(100 * $taskCpuMilliseconds / $taskDurationMilliseconds, 2)
    logicalProcessors = [Environment]::ProcessorCount
    treeCpuMilliseconds = $taskTreeCpuMilliseconds
    treeCpuPercentOfOneCore = [Math]::Round(100 * $taskTreeCpuMilliseconds / $taskDurationMilliseconds, 2)
    incomplete = $taskIncomplete
    scope = 'Launcher and descendant WebView2 processes; excludes Minecraft. Summed working sets may count shared pages more than once.'
    samples = $taskSamples
} | ConvertTo-Json -Depth 4
