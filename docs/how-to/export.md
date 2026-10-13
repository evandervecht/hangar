# Export the project list

Both formats print what's in the database right now, without scanning. Run `hangar scan` first if you want fresh data.

## To a spreadsheet

```sh
hangar list --csv > projects.csv
```

Open `projects.csv` in Excel, Numbers or Google Sheets. Notes with commas, quotes or line breaks are quoted correctly.

## To another tool or script

```sh
hangar list --json > projects.json
```

For example, the at-risk projects with `jq`:

```sh
hangar list --json | jq -r '.[] | select(.at_risk) | .path'
```

Every field is listed in [Project fields](../reference/project-fields.md#export-fields).
