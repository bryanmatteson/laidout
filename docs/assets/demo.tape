# Run scripts/record-readme-demo.py from the repository root.
Output docs/assets/demo.gif
Set Shell "bash"
Set FontSize 20
Set Width 1280
Set Height 700
Set Padding 26
Set Framerate 10
Set TypingSpeed 25ms
Set Theme "Catppuccin Mocha"
Hide
Type@0ms `alias laidout='"$README_DEMO_BIN"'; export PS1='laidout ❯ '; unset PROMPT_COMMAND; clear`
Enter
Show
Type@0ms "# One document, different widths"
Enter
Type "laidout json 80"
Enter
Sleep 4s
Type "laidout json 38"
Enter
Sleep 5s
Hide
Type@0ms "clear"
Enter
Show
Type@0ms "# Tables align by display columns, including Unicode"
Enter
Type "laidout table 60"
Enter
Sleep 4s
Type "laidout table 14"
Enter
Sleep 6s
