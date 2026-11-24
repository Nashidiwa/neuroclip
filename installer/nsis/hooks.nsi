; NeuroClip NSIS Installer & Uninstaller Hooks (AC-32.2)
; Runs under installMode: "currentUser" (HKCU) — no Administrator privileges required.

!macro NSIS_HOOK_POSTINSTALL
  ; Ensure Start Menu and Desktop shortcuts exist cleanly under current user
  Call CreateOrUpdateDesktopShortcut
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Clean temporary cache directory on uninstall while preserving user projects/ and logs/
  SetShellVarContext current
  RMDir /r "$LOCALAPPDATA\NeuroClip\cache"
!macroend
