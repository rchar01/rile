# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

Output artifacts/search.gif

Set Width 1280
Set Height 720
Set FontSize 18
Set Padding 12
Set CursorBlink false

Require target/debug/rile

Type "./target/debug/rile --visual-test --test-size 100x24 fixtures/visual/numbered.txt"
Enter
Wait+Screen /numbered.txt/
Sleep 500ms
Screenshot artifacts/frames/search-00-open.png
Sleep 500ms

Ctrl+S
Wait+Screen /I-search/
Sleep 500ms
Screenshot artifacts/frames/search-01-prompt.png
Sleep 500ms

Type "007"
Wait+Screen /I-search: 007/
Sleep 500ms
Screenshot artifacts/frames/search-02-match.png
Sleep 500ms

Ctrl+S
Sleep 1s
Screenshot artifacts/frames/search-03-repeat.png
Sleep 500ms

Enter
Sleep 1s
Screenshot artifacts/frames/search-04-accept.png
Sleep 500ms

Ctrl+X
Ctrl+C
