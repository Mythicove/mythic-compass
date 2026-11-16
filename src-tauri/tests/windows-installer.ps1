# This scenario installs into a fresh hosted runner's user profile and changes file associations.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Run installer acceptance on an isolated GitHub-hosted Windows runner.'
}

$config = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'src-tauri/target' }
$installers = @(Get-ChildItem -LiteralPath "$target/release/bundle/nsis" -Filter "*_$($config.version)_*-setup.exe")
if ($installers.Count -ne 1) { throw 'Expected one NSIS installer for the configured version.' }
$directory = Join-Path $env:RUNNER_TEMP 'compass-installer-acceptance'
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$($config.productName)"
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $uninstallKey)) {
    throw 'Installer acceptance requires a fresh installation directory and user profile.'
}
$extensionKey = 'HKCU:\Software\Classes\.mrpack'
$beforeAssociation = if (Test-Path -LiteralPath $extensionKey) { (Get-Item -LiteralPath $extensionKey).GetValue('') } else { $null }
$beforeAssociation = [string]$beforeAssociation
$application = $null
$installed = $false
try {
    $install = Start-Process -FilePath $installers[0].FullName -ArgumentList "/S /D=$directory" -Wait -PassThru
    if ($install.ExitCode -ne 0) { throw "NSIS installation failed: $($install.ExitCode)" }
    $installed = $true
    $entry = Get-ItemProperty -LiteralPath $uninstallKey
    $executable = Join-Path $directory $entry.MainBinaryName
    if (!(Test-Path -LiteralPath $executable)) { throw 'Installed executable is missing.' }
    if ($entry.DisplayVersion -ne $config.version) { throw 'Installed version differs from the release configuration.' }

    $programId = (Get-Item -LiteralPath $extensionKey).GetValue('')
    $command = (Get-Item -LiteralPath "HKCU:\Software\Classes\$programId\shell\open\command").GetValue('')
    if ($command -ne "`"$executable`" `"%1`"") { throw 'The mrpack association does not open the installed launcher with the selected file.' }
    $shortcutPath = Join-Path ([Environment]::GetFolderPath('Programs')) "$($config.bundle.windows.nsis.startMenuFolder)\$($config.productName).lnk"
    if (!(Test-Path -LiteralPath $shortcutPath)) { throw 'Start menu shortcut is missing.' }
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcutPath)
    if ($shortcut.TargetPath -ne $executable) { throw 'Start menu shortcut points to a different executable.' }

    $application = Start-Process -FilePath $executable -PassThru
    $windowDeadline = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 200
        $application.Refresh()
        if ($application.HasExited) { throw "Installed launcher exited during startup: $($application.ExitCode)" }
    } while (($application.MainWindowHandle -eq 0 -or $application.MainWindowTitle -ne $config.app.windows[0].title) -and $windowDeadline.Elapsed.TotalSeconds -lt 20)
    if ($application.MainWindowHandle -eq 0 -or $application.MainWindowTitle -ne $config.app.windows[0].title) { throw 'Installed launcher did not create its configured native window.' }
    $windowTitle = $application.MainWindowTitle
} finally {
    if ($application -and !$application.HasExited) {
        $application.Kill($true)
        $application.WaitForExit()
    }
    if ($installed) {
        $uninstall = Start-Process -FilePath "$directory\uninstall.exe" -ArgumentList "/S _?=$directory" -Wait -PassThru
        if ($uninstall.ExitCode -ne 0) { throw "NSIS uninstallation failed: $($uninstall.ExitCode)" }
    }
}

if (Test-Path -LiteralPath $executable) { throw 'Uninstallation left the launcher executable behind.' }
if (Test-Path -LiteralPath $uninstallKey) { throw 'Uninstallation left the add/remove programs entry behind.' }
if (Test-Path -LiteralPath $shortcutPath) { throw 'Uninstallation left the start menu shortcut behind.' }
$afterAssociation = if (Test-Path -LiteralPath $extensionKey) { (Get-Item -LiteralPath $extensionKey).GetValue('') } else { $null }
if ([string]$afterAssociation -ne $beforeAssociation) { throw 'Uninstallation did not restore the previous mrpack association.' }

@{
    version = $config.version
    installed = $true
    association = 'mrpack opens installed executable with selected file'
    start_menu = 'installed executable'
    native_window = $windowTitle
    uninstalled = $true
    association_restored = $true
    scope = 'Windows installer, shortcut, file association and native window; game launch and signed updates are separate acceptance steps'
} | ConvertTo-Json | Set-Content -LiteralPath "$env:RUNNER_TEMP/compass-installer-acceptance.json" -Encoding utf8
