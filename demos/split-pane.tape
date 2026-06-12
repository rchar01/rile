# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

Output artifacts/split-pane.gif

Set Width 1280
Set Height 720
Set FontSize 18
Set Padding 12
Set CursorBlink false

Require target/debug/rile

Type "./target/debug/rile --visual-test --test-size 120x32 fixtures/visual/split_left.txt"
Enter
Wait+Screen /Rile VISUAL/
Sleep 500ms
Screenshot artifacts/frames/split-pane-00-open.png
Sleep 500ms

Ctrl+X
Type "3"
Sleep 1s
Screenshot artifacts/frames/split-pane-01-C-x-3.png
Sleep 500ms

Ctrl+X
Type "o"
Sleep 1s
Screenshot artifacts/frames/split-pane-02-C-x-o.png
Sleep 500ms

Ctrl+X
Ctrl+F
Sleep 1s
Screenshot artifacts/frames/split-pane-03-C-x-C-f.png
Sleep 500ms
Type "fixtures/visual/split_right.txt"
Sleep 1s
Screenshot artifacts/frames/split-pane-04-path-entered.png
Sleep 500ms
Enter
Wait+Screen /split_right.txt/
Sleep 500ms
Screenshot artifacts/frames/split-pane-05-open-right.png
Sleep 500ms

Ctrl+N
Sleep 1s
Screenshot artifacts/frames/split-pane-06-C-n.png
Sleep 500ms
Ctrl+N
Sleep 1s
Screenshot artifacts/frames/split-pane-07-C-n.png
Sleep 500ms

Ctrl+X
Ctrl+C
