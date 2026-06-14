;; SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
;; SPDX-License-Identifier: GPL-3.0-or-later

(setq inhibit-splash-screen t)
(setq inhibit-startup-buffer-menu t)
(setq inhibit-startup-screen t)
(setq ring-bell-function #'ignore)
(setq use-dialog-box nil)
(setq use-file-dialog nil)

(when (fboundp 'menu-bar-mode)
  (menu-bar-mode -1))
(when (fboundp 'tool-bar-mode)
  (tool-bar-mode -1))
(when (fboundp 'scroll-bar-mode)
  (scroll-bar-mode -1))
(when (fboundp 'horizontal-scroll-bar-mode)
  (horizontal-scroll-bar-mode -1))

(defun rile-reference-add-elpa-package (name)
  "Add Debian ELPA package NAME to `load-path` when installed."
  (let ((elpa-root "/usr/share/emacs/site-lisp/elpa"))
    (when (file-directory-p elpa-root)
      (dolist (directory (directory-files elpa-root t (concat "^" name "-")))
        (when (file-directory-p directory)
          (add-to-list 'load-path directory))))))

(dolist (package '("compat" "vertico" "marginalia" "modus-themes"))
  (rile-reference-add-elpa-package package))

(require 'vertico)
(require 'marginalia)
(require 'modus-themes nil t)

(setq completion-styles '(basic substring))
(setq completion-cycle-threshold nil)
(setq read-file-name-completion-ignore-case nil)
(setq read-buffer-completion-ignore-case nil)

(when (member 'modus-vivendi-tinted (custom-available-themes))
  (load-theme 'modus-vivendi-tinted t))

(vertico-mode 1)
(marginalia-mode 1)
