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
  DetailPrint "Stopping ZonDPI Windows Service..."
  nsExec::ExecToLog '"$INSTDIR\zondpi-service.exe" stop'
  Pop $0
  Sleep 1000
  DetailPrint "Unregistering ZonDPI Windows Service..."
  nsExec::ExecToLog '"$INSTDIR\zondpi-service.exe" uninstall'
  Pop $0
  DetailPrint "ZonDPI service removed."
!macroend

!macro customInstall
  !insertmacro NSIS_HOOK_POSTINSTALL
!macroend

!macro customUnInstall
  !insertmacro NSIS_HOOK_PREUNINSTALL
!macroend
