# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

Output artifacts/movement.gif

Set Width 1280
Set Height 720
Set FontSize 18
Set Padding 12
Set CursorBlink false

Require target/debug/rile

Type "./target/debug/rile --visual-test --test-size 120x32 fixtures/visual/numbered.txt"
Enter
Wait+Screen /Rile VISUAL/
Sleep 500ms
Screenshot artifacts/frames/movement-00-open.png

Ctrl+N
Sleep 1s
Screenshot artifacts/frames/movement-01-C-n.png
Ctrl+N
Sleep 1s
Screenshot artifacts/frames/movement-02-C-n.png
Ctrl+F
Sleep 1s
Screenshot artifacts/frames/movement-03-C-f.png
Ctrl+F
Sleep 1s
Screenshot artifacts/frames/movement-04-C-f.png
Ctrl+P
Sleep 1s
Screenshot artifacts/frames/movement-05-C-p.png
Ctrl+A
Sleep 1s
Screenshot artifacts/frames/movement-06-C-a.png
Ctrl+E
Sleep 1s
Screenshot artifacts/frames/movement-07-C-e.png

Ctrl+X
Ctrl+C
