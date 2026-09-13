#requires -Version 7.2
# Offline derivation for the two immutable round-9 runs; never a live artifact reader.
# Hash recipe preserved from README.md at 45d96c7 (the parent of ef63d0e).
$ErrorActionPreference = 'Stop'
function Get-Utf8Hash([string]$Text) {
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text)))
}
foreach ($run in @('pr13r9ba', 'pr13r9bb')) {
    $directory = Join-Path $PSScriptRoot $run
    $seal = Get-Content -LiteralPath (Join-Path $directory 'seal.json') -Raw | ConvertFrom-Json
    foreach ($file in $seal.files.PSObject.Properties) {
        $actual = (Get-FileHash -LiteralPath (Join-Path $directory $file.Name) -Algorithm SHA256).Hash
        if ($actual -ne $file.Value) { throw "Hash mismatch: $run/$($file.Name)" }
    }
    $rows = @(Get-Content -LiteralPath (Join-Path $directory 'results.jsonl') | ForEach-Object { $_ | ConvertFrom-Json })
    $values = @($rows | ForEach-Object { $_.metrics.'continuity_recall_fraction_gap_short@5' } | Where-Object { $null -ne $_ })
    $mean = ($values | Measure-Object -Average).Average
    foreach ($row in $rows) { $row.latency_ms = 0; $row.run_id = '__RUN__' }
    $normalized = Get-Utf8Hash (ConvertTo-Json -InputObject $rows -Compress -Depth 100)
    $report = Get-Content -LiteralPath (Join-Path $directory 'report.json') -Raw | ConvertFrom-Json
    $content = Get-Utf8Hash (ConvertTo-Json -InputObject $report.content -Compress -Depth 100)
    if ($normalized -ne '4262058DFB0EC698BA66D772E84E052564BF1A942825AE31DDEDAB91478DA4FA') { throw 'Normalized-row hash differs from register' }
    if ($content -ne '41535684AA2A20A7D26F852FB03221C3C2EC51E6B4B52C54259C4DB6EDB0274E') { throw 'Report-content hash differs from register' }
    if ($values.Count -ne 8 -or $mean -ne 0.5208333333333334) { throw 'Short-gap recall@5 differs from register' }
    [ordered]@{
        run = $run
        normalized_rows_sha256 = $normalized
        report_content_sha256 = $content
        metric = 'continuity_recall_fraction_gap_short@5'
        numeric_values = $values
        numeric_rows = $values.Count
        sum = ($values | Measure-Object -Sum).Sum
        mean = $mean
    } | ConvertTo-Json -Depth 100
}
