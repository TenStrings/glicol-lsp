*Important*: this is a WIP/PoC at the moment.

Language server for [Glicol](https://glicol.org/).

# Current (maybe partial) support

- There is some support for the semantic tokens, which can be used to get some highlighting (at least in vscode). Although it's not necessarily fast. For editors with treesitter support it's better to just [directly the grammar](https://github.com/TenStrings/tree-sitter-glicol).
- Hover for nodes, which shows something similar to `help(node)`.
- Go to definition.
- Diagnostics from the pest parser, to get parsing errors in the editor. Other errors like using undefined references are not shown yet. 

# Setup

## From sources

### Install the binary.

```bash
git clone --recurse-submodules https://github.com/TenStrings/glicol-lsp
cargo install --path .
```

Make sure `glicol-lsp` it's available on your PATH now.

### vscode extension

First install [vsce](https://github.com/microsoft/vscode-vsce)

```bash
cd vscode-extension
npm install
vsce package
code --install-extension glicol-lsp-client-0.0.1.vsix
```
