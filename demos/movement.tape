# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

Output artifacts/movement.gif

Set Width 1280
Set Height 720
Set FontSize 18
Set Padding 12

Type "cargo run --locked -- --visual-test --test-size 80x24 fixtures/visual/numbered.txt"
Enter
Sleep 2s

Ctrl+N
Ctrl+N
Ctrl+F
Ctrl+F
Ctrl+P
Ctrl+A
Ctrl+E
Sleep 1s

Ctrl+X
Ctrl+C
