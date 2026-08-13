# LZ Language Support for VS Code

Syntax highlighting support for the Lang-Zone (LZ) programming language.

## Features

- Syntax highlighting for `.lz` files
- Comments (line `//` and block `/* */`)
- String literals (regular, f-strings, raw strings, multi-line)
- All LZ keywords, operators, and built-in types
- Decorator (`@xxx`) and attribute macro (`#!xxx`) highlighting
- Magic method (`__xxx__`) highlighting

## Installation

Copy this extension to your VS Code extensions folder:

```
%USERPROFILE%\.vscode\extensions\lz-lang\
```

Or install via VS Code command palette:

1. Press `Ctrl+Shift+P`
2. Run `Extensions: Install from VSIX...`
3. Select the `.vsix` file

## Language Features

| Feature | Support |
|---------|---------|
| Syntax Highlighting | Yes |
| Bracket Matching | Yes |
| Auto-closing Pairs | Yes |
| Comment Toggling | Yes |
| Indentation Rules | Yes |

## File Extension

`.lz` files are automatically recognized as LZ language files.