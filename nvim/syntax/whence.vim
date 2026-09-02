if exists("b:current_syntax")
  finish
endif

syntax match WhenceStop /\[\w\+:.*\]$/
syntax match WhenceTrunc /… \d\+ more/
syntax match WhenceLoc /\S\+:\d\+:\d\+/

highlight default link WhenceStop DiagnosticWarn
highlight default link WhenceTrunc Comment
highlight default link WhenceLoc Directory

let b:current_syntax = "whence"
