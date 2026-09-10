<#
.SYNOPSIS
Creates and pushes the next build tag (build-YYYY-MM-DD[_N]) to trigger the release workflow.

.DESCRIPTION
Looks at today's date and any existing build-YYYY-MM-DD tags:
- If no tag exists yet for today, creates "build-YYYY-MM-DD".
- If one or more tags already exist for today, creates the next running "_N" suffix
  (build-YYYY-MM-DD_1, then _2, etc).

.PARAMETER NoPush
Create the tag locally but don't push it (and therefore don't trigger a release).
#>
[CmdletBinding()]
param(
    [switch]$NoPush
)

$ErrorActionPreference = 'Stop'

# Sync with remote first: the release workflow deletes old tags via `gh release delete
# --cleanup-tag`, so the local repo can otherwise still have stale tags lying around.
git fetch origin --tags --prune --prune-tags | Out-Null

$today = Get-Date -Format 'yyyy-MM-dd'
$pattern = "^build-$([regex]::Escape($today))(?:_(\d+))?$"

$maxSuffix = -1
foreach ($existingTag in git tag) {
    if ($existingTag -match $pattern) {
        $suffix = if ($Matches[1]) { [int]$Matches[1] } else { 0 }
        if ($suffix -gt $maxSuffix) { $maxSuffix = $suffix }
    }
}

$newTag = if ($maxSuffix -lt 0) { "build-$today" } else { "build-${today}_$($maxSuffix + 1)" }

git tag $newTag
Write-Host "Created tag: $newTag"

if ($NoPush) {
    Write-Host "Skipping push (-NoPush given). Push manually with: git push origin $newTag"
} else {
    git push origin $newTag
    Write-Host "Pushed $newTag - the release workflow should start shortly."
}
