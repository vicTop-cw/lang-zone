# T0r61 收口判据：验证 embed tuple 序列化 + dict 序列化 + 嵌套解析零回归
# 用法：powershell -File scripts/check_embed_tuple.ps1
chcp 65001 > $null
[Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$fail = 0
$suites = @('embed_tuple_serialization','embed_dict_serialization','embed_dict_serialization_ext','embed_unify_r15','embed_list_serialization')

foreach ($s in $suites) {
    $out = & cargo test --test $s 2>&1
    $rc = $LASTEXITCODE
    $tail = ($out | Where-Object { $_ -match 'test result' } | Select-Object -Last 1)
    if ($rc -ne 0) {
        Write-Output "FAIL $s RC=$rc"
        Write-Output ($out | Select-Object -Last 3)
        $fail = 1
    } else {
        Write-Output "PASS $s : $tail"
    }
}

if ($fail -ne 0) { Write-Output "CHECK_FAIL"; exit 1 }
Write-Output "CHECK_PASS"
exit 0
