# Renamed from pg-pseudo-mcp

What changed when pg-pseudo-mcp became scarif-database-mcp, and how to finish the migration.

This project was called `pg-pseudo-mcp` up to 0.2.0. From 0.3.0 the crate, the binary and the release archives are named `scarif-database-mcp`, and the MCP server announces itself as `scarif`. The old names still work for this version, with a warning, and will be removed later.

| | Before (0.2.x) | From 0.3.0 |
|---|---|---|
| Binary and archives | `pg-pseudo-mcp`, `pg-pseudo-mcp-<target>.tar.gz` | `scarif-database-mcp`, `scarif-database-mcp-<target>.tar.gz` |
| MCP server name | `pg-pseudo-mcp` | `scarif` |
| Environment variables | `PG_PSEUDO_CONNECTIONS_FILE`, `PG_PSEUDO_COLUMNS_FILE`, `PG_PSEUDO_SSH_CONFIG`, `PG_PSEUDO_SSH_KNOWN_HOSTS`, `PG_PSEUDO_DBEAVER_HOME`, `PG_PSEUDO_DATA_DIR` | the same names with the `SCARIF_` prefix |
| Config directory | `~/.config/pg-pseudo/` | `~/.config/scarif/` (or `$XDG_CONFIG_HOME/scarif/`) |
| Data directory | `~/Library/Application Support/pg-pseudo-mcp` (macOS), `~/.local/share/pg-pseudo-mcp` (Linux) | `.../scarif-database-mcp` in the same parent |

Fallbacks, none of which copies or moves anything:

- **Variables**: an old `PG_PSEUDO_*` variable is used only when its `SCARIF_*` counterpart is unset, and the server writes one deprecation warning per old variable to stderr (never its value).
- **Config directory**: `connections.toml` and `columns.toml` are each read from `~/.config/scarif/`; when a file is absent there but present in `~/.config/pg-pseudo/`, the old one is read and a `config_location_deprecated` warning appears on stderr and in `server_info`. `connections import-dbeaver` always writes to the new directory.
- **Data directory**: it holds the pseudonym secret, so losing it changes every pseudonym. When the new directory does not exist and the old one does, the old one keeps being used, reported by a `data_directory_deprecated` warning on stderr and in `server_info`. `mapping purge` acts on the directory in use. When both directories exist and the old one holds a secret, the old one keeps being used, since its secret decides the pseudonyms already seen, and a `data_directory_conflict` warning names both directories with the two ways out: remove the new one then move the old one into place to keep the known pseudonyms, or remove the old one to accept new pseudonyms. Otherwise the new directory wins. `SCARIF_DATA_DIR` (or `PG_PSEUDO_DATA_DIR`) is always used as given.

To finish the migration by hand, with the server stopped (macOS shown; on Linux use `~/.local/share`):

```bash
mv ~/Library/Application\ Support/pg-pseudo-mcp ~/Library/Application\ Support/scarif-database-mcp
```

Then move `~/.config/pg-pseudo` to `~/.config/scarif`, rename the variables in your MCP registration (`claude mcp add scarif -- scarif-database-mcp`), and update your deny rules (the old and new paths are both in [claude-code-deny-rules.json](claude-code-deny-rules.json)).
