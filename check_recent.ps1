$latest = (Get-ChildItem -Path . -Recurse -File -Force -ErrorAction SilentlyContinue | Where-Object { $_.Extension -in '.rs','.py','.pyx','.md','.toml','.lz' -and $_.FullName -notmatch '\\target\\|\\node_modules\\|\\.git\\' } | Sort-Object LastWriteTime -Descending | Select-Object -First 1).LastWriteTime
$now = Get-Date
if ($null -eq $latest) {
    Write-Output "NO_SOURCE_FILE_FOUND"
    Write-Output "CONTINUE_PROCEED"
    exit 0
}
$diff = ($now - $latest).TotalMinutes
Write-Output "最新源文件修改时间: $latest"
Write-Output "当前时间: $now"
Write-Output "间隔分钟数: $([math]::Round($diff, 2))"
if ($diff -lt 5) { Write-Output "TRIGGERED_SKIP"; exit 0 } else { Write-Output "CONTINUE_PROCEED"; exit 0 }
