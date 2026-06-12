# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

Output artifacts/split-pane.gif

Set Width 1280
Set Height 720
Set FontSize 18
Set Padding 12

Type "cargo run --locked -- --visual-test --test-size 80x24 fixtures/visual/split_left.txt"
Enter
Sleep 2s

Ctrl+X
Type "3"
Sleep 500ms

Ctrl+X
Ctrl+O
Sleep 500ms

Ctrl+X
Ctrl+F
Type "fixtures/visual/split_right.txt"
Enter
Sleep 1s

Ctrl+N
Ctrl+N
Sleep 1s


Ctrl+X
Ctrl+C
