# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

Output artifacts/open-edit-save.gif

Set Width 1280
Set Height 720
Set FontSize 18
Set Padding 12
Set CursorBlink false

Require target/debug/rile

Type "rm -f artifacts/open-edit-save.txt && ./target/debug/rile --visual-test --test-size 100x24 artifacts/open-edit-save.txt"
Enter
Wait+Screen /open-edit-save.txt/
Sleep 500ms
Screenshot artifacts/frames/open-edit-save-00-open.png
Sleep 500ms

Type "hello from visual demo"
Sleep 1s
Screenshot artifacts/frames/open-edit-save-01-insert-ascii.png
Sleep 500ms

Enter
Type "second line from visual demo"
Sleep 1s
Screenshot artifacts/frames/open-edit-save-02-insert-second-line.png
Sleep 500ms

Ctrl+X
Ctrl+S
Wait+Screen /Wrote/
Sleep 500ms
Screenshot artifacts/frames/open-edit-save-03-saved.png
Sleep 500ms

Ctrl+X
Ctrl+C
