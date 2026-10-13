# CLI

```
hangar [scan | list [--json | --csv]]
```

| Command | Does | Output |
|---|---|---|
| `hangar` | scans, then opens the window | none |
| `hangar scan` | imports new and changed session logs, scans the roots, refreshes project fields and git health | `imported N log files, M projects (data: <folder>)` |
| `hangar list` | prints every visible project from the database, without scanning | a text table, newest activity first |
| `hangar list --json` | the same rows as a JSON array | see [export fields](project-fields.md#export-fields) |
| `hangar list --csv` | the same rows as RFC 4180 CSV with a header row | see [export fields](project-fields.md#export-fields) |
| anything else | prints usage | exit code 2 |

`hangar list` columns: `PROJECT` (full path, `(missing)` if the folder is gone), `STATUS`, `GIT`, `TOOLS`, `SESSIONS`, `ACTIVE`, `7 DAYS`, `LAST (UTC)`, `ORIGIN`.

Errors (unreadable config, database problems) are printed to stderr with exit code 1.
