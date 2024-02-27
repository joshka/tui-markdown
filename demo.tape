# VHS Tape (see https://github.com/charmbracelet/vhs)
Output "target/demo.gif"
Set Theme "Aardvark Blue"
Set Width 800
Set Height 600
Hide
Type "cargo run -- markdown-reader/TEST.md"
Enter
Sleep 2s
Show
Sleep 1s
Down@100ms 40
Sleep 2s
Type "g"
Sleep 1s
Hide
Type "q"
Sleep 500ms
