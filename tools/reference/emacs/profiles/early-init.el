;; SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
;; SPDX-License-Identifier: GPL-3.0-or-later

;; Keep reference captures minimal from Emacs' first frame.
(setq inhibit-splash-screen t)
(setq inhibit-startup-buffer-menu t)
(setq inhibit-startup-screen t)
(setq inhibit-default-init t)
(setq package-enable-at-startup nil)
(setq use-dialog-box nil)
(setq use-file-dialog nil)
(setq visible-cursor nil)

(when (fboundp 'menu-bar-mode)
  (menu-bar-mode -1))
(when (fboundp 'tool-bar-mode)
  (tool-bar-mode -1))
(when (fboundp 'scroll-bar-mode)
  (scroll-bar-mode -1))
(when (fboundp 'horizontal-scroll-bar-mode)
  (horizontal-scroll-bar-mode -1))
(when (fboundp 'blink-cursor-mode)
  (blink-cursor-mode -1))
