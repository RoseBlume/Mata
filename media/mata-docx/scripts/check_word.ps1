# Optional Windows smoke test requiring a licensed local copy of Microsoft Word.
param([Parameter(Mandatory=$true)][string]$Path)
$word = New-Object -ComObject Word.Application
$word.Visible = $false
try {
    $file = (Resolve-Path $Path).Path
    $doc = $word.Documents.Open($file, $false, $true)
    try {
        if ($doc.Paragraphs.Count -lt 1) { throw 'Word opened zero paragraphs' }
        Write-Host "Word opened file: $($doc.Paragraphs.Count) paragraphs"
    } finally { $doc.Close($false) }
} finally { $word.Quit() }
