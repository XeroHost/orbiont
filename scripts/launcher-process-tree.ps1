function Get-LauncherProcessIds {
    param([int] $RootId, [object[]] $Snapshot)
    $taskIds = [Collections.Generic.HashSet[int]]::new()
    [void] $taskIds.Add($RootId)
    do {
        $taskAdded = $false
        foreach ($taskEntry in $Snapshot) {
            if ($taskEntry.Name -eq 'msedgewebview2.exe' -and $taskIds.Contains([int] $taskEntry.ParentProcessId)) {
                if ($taskIds.Add([int] $taskEntry.ProcessId)) { $taskAdded = $true }
            }
        }
    } while ($taskAdded)
    return @($taskIds | Sort-Object)
}
