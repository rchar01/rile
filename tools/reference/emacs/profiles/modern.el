;; SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
;; SPDX-License-Identifier: GPL-3.0-or-later

(setq ring-bell-function #'ignore)

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
