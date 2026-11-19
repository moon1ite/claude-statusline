# Claude Code Statusline

A real-time statusline plugin for [Claude Code](https://claude.ai/claude-code) that displays context, cost, usage limits, the active skill, and todo progress.

Agent and tool activity are not shown, because Claude Code now displays them natively (the subagent panel under the prompt and live tool lines).

![Statusline Preview](docs/preview.png)

## Features

- **Context Window**: Visual progress bar showing token usage
- **Session Cost**: Running cost of the current session
- **Git Status**: Current branch with file counts and line changes
- **Skill Tracking**: Show which skill Claude is currently using (brainstorming, TDD, etc.)
- **Todo Progress**: Current task and completion status (done/total)

## Preview

```
██████░░░░░░░░░ 42% ($0.0234) | main (3 files +45 -12) | my-project | Claude Opus 4
 ⟳ Fix auth bug (2/5) |  ✓ brainstorming
```

**Line 1** (Bash): Context bar, cost, git status, directory, model
**Line 2** (Rust): Todos, skills (hidden when both are empty)

## Requirements

- **macOS** (Apple Silicon)
- **Rust** (for building the binary)
- **jq** (for JSON parsing)
- **Claude Code** CLI

## Installation

### Quick Install

```bash
git clone https://github.com/moon1ite/claude-statusline.git
cd claude-statusline
./install.sh
```

### Manual Install

1. **Build the Rust binary:**
   ```bash
   cargo build --release
   ```

2. **Copy the binary to Claude directory:**
   ```bash
   mkdir -p ~/.claude/bin
   cp target/release/claude-status ~/.claude/bin/
   chmod +x ~/.claude/bin/claude-status
   ```

3. **Configure Claude Code** by adding to `~/.claude/settings.json`:
   ```json
   {
     "statusLine": {
       "type": "command",
       "command": "/path/to/claude-statusline/statusline.sh",
       "padding": 0
     }
   }
   ```
   Replace `/path/to/claude-statusline` with where you cloned the repo.

4. **Restart Claude Code** to see the statusline.

## Uninstallation

```bash
./uninstall.sh
```

Or manually:
```bash
rm ~/.claude/bin/claude-status
# Remove "statusLine" from ~/.claude/settings.json
```

## Configuration

### Color Scheme

The plugin uses the [Catppuccin Mocha](https://github.com/catppuccin/catppuccin) color palette:

| Element | Color | Hex |
|---------|-------|-----|
| Section icons (, , ) | Lavender | `#b4befe` |
| Complete (✓) | Green | `#a6e3a1` |
| Running (⟳) | Yellow | `#f9e2af` |
| Error (✗) | Red | `#f38ba8` |
| Separators | Gray | Standard terminal gray |

### Nerd Fonts

This plugin uses [Nerd Font](https://www.nerdfonts.com/) icons. Make sure your terminal uses a Nerd Font for proper icon rendering:

- `` - Checkbox (todos)
- `` - Lightning bolt (skills)
- `` - Spinner (running)
- `` - Check (complete)
- `` - X (error)

**Recommended fonts:** JetBrainsMono Nerd Font, FiraCode Nerd Font, Hack Nerd Font

## Architecture

```
claude-statusline/         # (your cloned repo)
├── statusline.sh          # Bash orchestrator (referenced by settings.json)
│   ├── Line 1: Context, cost, git, directory, model (via jq)
│   └── Line 2: Calls claude-status binary
├── src/main.rs            # Rust source code
├── install.sh             # Installer script
└── uninstall.sh           # Uninstaller script

~/.claude/
├── bin/
│   └── claude-status      # Rust binary (installed here)
└── settings.json          # Points to repo's statusline.sh
```

### How It Works

1. **Claude Code** calls `statusline.sh` periodically, passing JSON via stdin with model info, cost, and context window data.

2. **statusline.sh** (Bash):
   - Parses stdin JSON for Line 1 data
   - Gets git status from the current directory
   - Calls the Rust binary with the transcript path

3. **claude-status** (Rust):
   - Parses the JSONL transcript file
   - Tracks Skill invocations
   - Tracks TaskCreate/TaskUpdate calls
   - Outputs formatted Line 2

### Transcript Parsing

The Rust binary implements event sourcing over the Claude Code transcript:

| Event | Action |
|-------|--------|
| `Skill` tool_use | Create/update skill entry (deduplicated by name) |
| `Skill` tool_result | Mark skill complete |
| `TaskCreate` | Add todo (sequential ID) |
| `TaskUpdate` | Update todo status |
| `user` message (top-level) | Mark pending reset |
| `assistant` message (top-level) | Reset skills/todos for new turn |

**Per-turn tracking**: Skills and todos reset when you send a new message and Claude starts responding, so you only see the current turn's activity.

## Troubleshooting

### Icons not displaying
- Ensure your terminal uses a Nerd Font
- Test with: `echo -e "\uf110 \uf00c \uf00d"` (should show spinner, check, x)

### Statusline not appearing
1. Check Claude Code settings: `cat ~/.claude/settings.json | jq .statusLine`
2. Verify binary exists: `ls -la ~/.claude/bin/claude-status`
3. Check statusline.sh path in settings.json points to your repo
4. Test manually: `echo '{}' | /path/to/your/repo/statusline.sh`

### Skills/todos disappearing
- This is expected! Activity resets at the start of each turn
- They persist until you send a new message and Claude responds

### Build errors
- Ensure Rust is installed: `cargo --version`
- Try: `rustup update`

## Development

### Build from source
```bash
cargo build --release
```

### Run tests
```bash
cargo test
```

### Debug output
```bash
~/.claude/bin/claude-status /path/to/transcript.jsonl
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by [claude-hud](https://github.com/jarrodwatts/claude-hud)
- Color scheme: [Catppuccin](https://github.com/catppuccin/catppuccin)
- Icons: [Nerd Fonts](https://www.nerdfonts.com/)
