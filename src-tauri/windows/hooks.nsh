; Scrub My Disks: NSIS hooks. The uninstaller removes the scheduled Smart Care task.
!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec '"$SYSDIR\schtasks.exe" /Delete /TN "Scrub My Disks Smart Care" /F'
  Pop $0
!macroend
