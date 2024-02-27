# VHS Tape (see https://github.com/charmbracelet/vhs)
Output "target/demo.gif"
Set Theme "Aardvark Blue"
Set Width 800
Set Height 600
Hide
Type "cargo run -- markdown-reader/TEST.md"
Enter
Sleep 4s
Show
Sleep 4s
Space@4s 4
# Sleep 2s
Type "g"
Hide
Type "q"
Sleep 500ms
