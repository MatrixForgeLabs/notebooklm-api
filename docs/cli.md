# CLI Usage

## Global Options

All commands support:
- `--storage <PATH>`: storage_state.json path
- `-v/--verbose` (repeatable)
- `--output <json|table|tsv>`
- `--quiet`

## Command Tree

- `notebooklm auth-status`
- `notebooklm notebook <subcommand>`
- `notebooklm source <notebook_id> <subcommand>`
- `notebooklm artifact <notebook_id> <subcommand>`
- `notebooklm research <notebook_id> <subcommand>`
- `notebooklm settings <subcommand>`
- `notebooklm share <notebook_id> <subcommand>`
- `notebooklm chat <notebook_id> <question>`
- `notebooklm chat-history <notebook_id>`
- aliases: `notebooklm list`, `notebooklm create <title>`

## Notebook

```bash
notebooklm notebook list
notebooklm notebook create "Title"
notebooklm notebook get <id>
notebooklm notebook rename <id> "New Title"
notebooklm notebook delete <id>
notebooklm notebook summary <id>
```

## Source

```bash
notebooklm source <nb> list
notebooklm source <nb> add-url "https://..."
notebooklm source <nb> get <source_id>
notebooklm source <nb> rename <source_id> "New"
notebooklm source <nb> refresh <source_id>
notebooklm source <nb> fulltext <source_id>
notebooklm source <nb> delete <source_id>
```

## Artifact Generation

```bash
# Audio
notebooklm artifact <nb> generate-audio \
  --language en \
  --audio-format deep-dive \
  --audio-length long

# Video
notebooklm artifact <nb> generate-video \
  --video-format explainer \
  --video-style whiteboard

# Report
notebooklm artifact <nb> generate-report \
  --report-format briefing-doc

# Custom report
notebooklm artifact <nb> generate-report \
  --report-format custom \
  --custom-prompt "Write for executives"

# Quiz / Flashcards
notebooklm artifact <nb> generate-quiz --quantity fewer --difficulty hard
notebooklm artifact <nb> generate-flashcards --quantity standard --difficulty medium

# Infographic / Slides / Data table
notebooklm artifact <nb> generate-infographic --orientation portrait --detail-level detailed
notebooklm artifact <nb> generate-slide-deck --slide-format presenter-slides --slide-length short
notebooklm artifact <nb> generate-data-table --instructions "Columns: date, metric, value"

# Mind map
notebooklm artifact <nb> generate-mind-map
```

## Artifact Download / Export

```bash
notebooklm artifact <nb> download-audio out.mp3
notebooklm artifact <nb> download-video out.mp4
notebooklm artifact <nb> download-report out.md
notebooklm artifact <nb> download-data-table out.csv
notebooklm artifact <nb> download-quiz quiz.json --format json
notebooklm artifact <nb> download-flashcards cards.md --format markdown
notebooklm artifact <nb> download-mind-map map.json --format pretty-json

notebooklm artifact <nb> export-report <artifact_id> --title "Export"
notebooklm artifact <nb> export-data-table <artifact_id> --title "Export"
notebooklm artifact <nb> export --artifact-id <id> --export-type report
```

## Research

```bash
notebooklm research <nb> start "query" --source web --mode fast
notebooklm research <nb> poll
notebooklm research <nb> import <task_id> --url https://a --title "A"
```

## Share

```bash
notebooklm share <nb> status
notebooklm share <nb> set-public true
notebooklm share <nb> set-view-level chat-only
notebooklm share <nb> add-user person@example.com editor
notebooklm share <nb> update-user person@example.com viewer
notebooklm share <nb> remove-user person@example.com
```

## Output Modes

- `json`: machine-readable
- `tsv`: tab-separated text output
- `table`: currently aliased to TSV formatting

Use `--quiet` to suppress non-essential status lines where supported.
