# CLI

```
hangar [scan | list]
```

| Command | Does | Output |
|---|---|---|
| `hangar` | scans, then opens the window | none |
| `hangar scan` | imports new and changed session logs, scans the roots, refreshes project fields | `imported N log files, M projects (data: <folder>)` |
| `hangar list` | prints every visible project from the database without scanning | one row per project, newest activity first |
| anything else | prints usage | exit code 2 |

`hangar list` columns: `PROJECT` (full path, `(missing)` if the folder is gone), `STATUS`, `TOOLS`, `SESSIONS`, `ACTIVE`, `7 DAYS`, `LAST (UTC)`, `ORIGIN`. See [Project fields](project-fields.md) for what each means.

Errors (unreadable config, database problems) are printed to stderr with exit code 1.
