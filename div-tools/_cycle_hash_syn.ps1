Get-ChildItem -Path SYNTAX -Recurse -Filter *.md | ForEach-Object {
  $h = (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower()
  Write-Output "$h  $($_.FullName)"
}
