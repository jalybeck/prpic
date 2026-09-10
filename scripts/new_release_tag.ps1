<#
.SYNOPSIS
Creates and pushes the next build tag (build-YYYY-MM-DD[_N]) to trigger the release workflow.

.DESCRIPTION
Looks at today's date and any existing build-YYYY-MM-DD_N tags:
- If no tag exists yet for today, creates "build-YYYY-MM-DD_1".
- Otherwise creates the next running "_N" suffix (_2, _3, etc).

.PARAMETER NoPush
Create the tag locally but don't push it (and therefore don't trigger a release).
#>
[CmdletBinding()]
param(
    [switch]$NoPush
)

$ErrorActionPreference = 'Stop'

# Query the remote directly instead of `git fetch --prune-tags`: the release workflow
# deletes old tags on GitHub, and pruning locally would also wipe out any tag created here
# with -NoPush before it gets pushed.
$today = Get-Date -Format 'yyyy-MM-dd'
$pattern = "^build-$([regex]::Escape($today))_(\d+)$"

$maxSuffix = 0
foreach ($line in git ls-remote --tags origin) {
    $tagName = ($line -split "`t")[1] -replace '^refs/tags/', '' -replace '\^\{\}$', ''
    if ($tagName -match $pattern) {
        $suffix = [int]$Matches[1]
        if ($suffix -gt $maxSuffix) { $maxSuffix = $suffix }
    }
}

$newTag = "build-${today}_$($maxSuffix + 1)"

git tag $newTag
Write-Host "Created tag: $newTag"

if ($NoPush) {
    Write-Host "Skipping push (-NoPush given). Push manually with: git push origin $newTag"
} else {
    git push origin $newTag
    Write-Host "Pushed $newTag - the release workflow should start shortly."
}
