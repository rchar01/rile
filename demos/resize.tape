# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

Output artifacts/resize.gif

Set Width 1280
Set Height 720
Set FontSize 18
Set Padding 12
Set CursorBlink false

Require target/debug/rile

Type "./target/debug/rile --visual-test --test-size 48x12 fixtures/visual/long_lines.txt"
Enter
Wait+Screen /Rile VISUAL/
Sleep 500ms
Screenshot artifacts/frames/resize-00-narrow-open.png
Sleep 500ms

Ctrl+E
Sleep 1s
Screenshot artifacts/frames/resize-01-horizontal-scroll.png
Sleep 500ms

Ctrl+X
Ctrl+C
Sleep 500ms

Type "./target/debug/rile --visual-test --test-size 80x8 fixtures/visual/numbered.txt"
Enter
Wait+Screen /numbered.txt/
Sleep 500ms
Screenshot artifacts/frames/resize-02-short-open.png
Sleep 500ms

Ctrl+N
Ctrl+N
Ctrl+N
Ctrl+N
Ctrl+N
Ctrl+N
Ctrl+N
Ctrl+N
Ctrl+N
Ctrl+N
Sleep 1s
Screenshot artifacts/frames/resize-03-vertical-scroll.png
Sleep 500ms

Ctrl+X
Ctrl+C
