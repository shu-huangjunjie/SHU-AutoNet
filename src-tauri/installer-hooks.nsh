!define SHU_INSTALLER_HOOK_DIR "${__FILEDIR__}"

!macro NSIS_HOOK_POSTINSTALL
  SetOutPath "$INSTDIR"
  File /oname=LICENSE "${SHU_INSTALLER_HOOK_DIR}\..\LICENSE"
!macroend

; The app uses the same Run value name as older portable releases. Only remove
; it when it points at this installed executable, and preserve it on updates.
!macro NSIS_HOOK_PREUNINSTALL
  Delete "$INSTDIR\LICENSE"
  ${If} $UpdateMode <> 1
    ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "SHU_AutoNet"
    ${If} $0 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
    ${OrIf} $0 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --minimized"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "SHU_AutoNet"
    ${EndIf}
  ${EndIf}
!macroend
