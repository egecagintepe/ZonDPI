; ZonDPI NSIS Installer Hooks
; Handles Windows Service registration and startup on install, cleanup on uninstall

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registering ZonDPI Windows Service..."
  nsExec::ExecToLog '"$INSTDIR\zondpi-service.exe" install'
  Pop $0
  DetailPrint "ZonDPI service registration result: $0"
  DetailPrint "Configuring ZonDPI Windows Service startup to Automatic..."
  nsExec::ExecToLog 'sc.exe config ZonDPI start= auto'
  Pop $0
  DetailPrint "ZonDPI service startup config result: $0"
  DetailPrint "Starting ZonDPI Windows Service..."
  nsExec::ExecToLog '"$INSTDIR\zondpi-service.exe" start'
  Pop $0
  DetailPrint "ZonDPI service start result: $0"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Stopping ZonDPI application and worker processes..."
  nsExec::ExecToLog 'taskkill.exe /F /IM "goodbyedpi.exe" /T'
  nsExec::ExecToLog 'taskkill.exe /F /IM "ciadpi.exe" /T'
  nsExec::ExecToLog 'taskkill.exe /F /IM "zondpi-engine-worker.exe" /T'
  nsExec::ExecToLog 'taskkill.exe /F /IM "zondpi-gui.exe" /T'
  nsExec::ExecToLog 'taskkill.exe /F /IM "zondpi-cli.exe" /T'
  Sleep 500

  DetailPrint "Stopping ZonDPI Windows Service..."
  nsExec::ExecToLog '"$INSTDIR\zondpi-service.exe" stop'
  Pop $0
  Sleep 500

  DetailPrint "Unregistering ZonDPI Windows Service and cleaning drivers..."
  nsExec::ExecToLog '"$INSTDIR\zondpi-service.exe" uninstall'
  Pop $0
  Sleep 500

  DetailPrint "Purging WinDivert kernel drivers from Windows SCM..."
  nsExec::ExecToLog 'net.exe stop WinDivert /y'
  nsExec::ExecToLog 'sc.exe delete WinDivert'
  nsExec::ExecToLog 'net.exe stop WinDivert14 /y'
  nsExec::ExecToLog 'sc.exe delete WinDivert14'
  nsExec::ExecToLog 'net.exe stop WinDivert22 /y'
  nsExec::ExecToLog 'sc.exe delete WinDivert22'
  Sleep 500

  DetailPrint "ZonDPI service and WinDivert kernel drivers removed cleanly."
!macroend

!macro customInstall
  !insertmacro NSIS_HOOK_POSTINSTALL
!macroend

!macro customUnInstall
  !insertmacro NSIS_HOOK_PREUNINSTALL
!macroend
