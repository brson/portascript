;;; portascript-mode.el --- Major mode for Portascript (.psc) files -*- lexical-binding: t; -*-

(defgroup portascript nil
  "Major mode for editing Portascript files."
  :group 'languages)

(defcustom portascript-indent-offset 4
  "Indentation width for Portascript."
  :type 'integer
  :group 'portascript)

;; Syntax table: # comments, string delimiters.
(defvar portascript-mode-syntax-table
  (let ((st (make-syntax-table)))
    (modify-syntax-entry ?# "<" st)
    (modify-syntax-entry ?\n ">" st)
    (modify-syntax-entry ?\" "\"" st)
    (modify-syntax-entry ?\' "\"" st)
    (modify-syntax-entry ?_ "_" st)
    (modify-syntax-entry ?\{ "(}" st)
    (modify-syntax-entry ?\} "){" st)
    (modify-syntax-entry ?\[ "(]" st)
    (modify-syntax-entry ?\] ")[" st)
    (modify-syntax-entry ?\( "()" st)
    (modify-syntax-entry ?\) ")(" st)
    st))

;; Font-lock keywords.
(defvar portascript-keywords
  '("let" "mut" "if" "elif" "else" "for" "in" "while"
    "break" "continue" "fn" "return" "match" "try" "run" "exec"
    "and" "or" "not" "env"))

(defvar portascript-constants
  '("true" "false"))

(defvar portascript-types
  '("str" "int" "float" "bool" "list" "map"))

(defvar portascript-builtin-functions
  '("len" "split" "join" "trim" "starts_with" "ends_with"
    "contains" "replace" "upper" "lower" "lines"
    "glob" "read" "write" "append_file" "tempfile"
    "range" "typeof" "error" "exit" "print"
    "eprintln" "eprint" "pid" "command_exists"
    "append" "keys" "has_key"))

(defvar portascript-font-lock-keywords
  (let ((kw-re (regexp-opt portascript-keywords 'symbols))
        (const-re (regexp-opt portascript-constants 'symbols))
        (type-re (regexp-opt portascript-types 'symbols))
        (builtin-fn-re (regexp-opt portascript-builtin-functions 'symbols)))
    `(;; Keywords.
      (,kw-re . font-lock-keyword-face)
      ;; Constants.
      (,const-re . font-lock-constant-face)
      ;; Function definitions.
      ("\\<fn\\s-+\\([a-zA-Z_][a-zA-Z0-9_]*\\)" 1 font-lock-function-name-face)
      ;; Type annotations (after : or ->).
      (,type-re . font-lock-type-face)
      ;; Builtin functions.
      (,builtin-fn-re . font-lock-builtin-face)
      ;; path.* namespace functions.
      ("\\<path\\.[a-zA-Z_]+" . font-lock-builtin-face)
      ;; String interpolation {expr} inside double-quoted strings.
      ("{[^}]+}" . font-lock-variable-name-face)
      ;; The ? error-suppression operator at end of command.
      ("\\s-\\(\\?\\)\\s-*$" 1 font-lock-warning-face)
      ;; Numbers.
      ("\\<[0-9]+\\(?:\\.[0-9]+\\)?\\>" . font-lock-constant-face))))

;; Indentation.
(defun portascript-indent-line ()
  "Indent current line of Portascript code."
  (interactive)
  (let ((indent (portascript--calc-indent)))
    (when indent
      (if (<= (current-column) (current-indentation))
          (indent-line-to indent)
        (save-excursion (indent-line-to indent))))))

(defun portascript--calc-indent ()
  "Calculate indentation for the current line."
  (save-excursion
    (beginning-of-line)
    (let ((cur-line (string-trim (thing-at-point 'line t))))
      ;; Closing brace: dedent relative to matching open.
      (if (string-prefix-p "}" cur-line)
          (progn
            (portascript--goto-prev-nonblank)
            (max 0 (- (current-indentation)
                      (if (string-match-p "[{]\\s-*$"
                                          (thing-at-point 'line t))
                          0
                        portascript-indent-offset))))
        ;; Normal line: base on previous line.
        (if (bobp)
            0
          (portascript--goto-prev-nonblank)
          (let ((prev-indent (current-indentation))
                (prev-line (string-trim (thing-at-point 'line t))))
            (if (string-match-p "[{]\\s-*$" prev-line)
                (+ prev-indent portascript-indent-offset)
              prev-indent)))))))

(defun portascript--goto-prev-nonblank ()
  "Move point to the previous non-blank, non-comment line."
  (forward-line -1)
  (while (and (not (bobp))
              (looking-at-p "^\\s-*\\(?:#.*\\)?$"))
    (forward-line -1)))

;; Mode definition.
;;;###autoload
(define-derived-mode portascript-mode prog-mode "Portascript"
  "Major mode for editing Portascript (.psc) files."
  :syntax-table portascript-mode-syntax-table
  (setq-local comment-start "# ")
  (setq-local comment-end "")
  (setq-local indent-line-function #'portascript-indent-line)
  (setq-local font-lock-defaults '(portascript-font-lock-keywords)))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.psc\\'" . portascript-mode))

(provide 'portascript-mode)
;;; portascript-mode.el ends here
