# F:ist agent skills

This directory ships [Agent Skills](https://agentskills.io/specification)-format instructions that teach coding agents how to configure and operate [F:ist](https://github.com/Squirreljetpack/fist). The same files work in Codex, pi, OpenCode, and Claude Code; only the discovery directory differs. When installing, each file must be saved as `SKILL.md` inside a folder named after the skill.

| File                               | Skill               | Covers                                                                                       |
| ---------------------------------- | ------------------- | -------------------------------------------------------------------------------------------- |
| [`ACTIONS.md`](./ACTIONS.md)       | `fist-menu-actions` | Menu actions: TOML schema, strategies, conditions, the Lua environment, queue integration    |
| [`LESSFILTER.md`](./LESSFILTER.md) | `fist-lessfilter`   | `lessfilter.toml`: presets, scored rules and patterns, builtin/custom actions, categories    |
| [`LAUNCH.md`](./LAUNCH.md)         | `fist-launch`       | CLI pane launching: subcommands, visibility/sort flags, fd/rg passthrough, output formatting |
| [`USAGE.md`](./USAGE.md)           | `fist-usage`        | Everyday TUI operation: navigation, selection, file ops, queue, stashes, previews, options   |

## Install with curl

The commands below download reviewed files without executing remote content. Pick your host's root, then install any subset of the four skills:

```sh
URL='https://raw.githubusercontent.com/Squirreljetpack/fist/main/assets/plugins'

# OpenAI Codex
ROOT="$HOME/.agents/skills"
# pi coding agent
# ROOT="$HOME/.pi/agent/skills"
# OpenCode
# ROOT="$HOME/.config/opencode/skills"
# Claude Code
# ROOT="$HOME/.claude/skills"

for pair in \
  fist-menu-actions:ACTIONS \
  fist-lessfilter:LESSFILTER \
  fist-launch:LAUNCH \
  fist-usage:USAGE
do
  name=${pair%%:*} file=${pair##*:}
  mkdir -p "$ROOT/$name"
  curl -fsSL "$URL/$file.md" -o "$ROOT/$name/SKILL.md"
done
```

Restart the host, or reload its skills, after installing or updating. To update an existing installation, re-run the same commands; they overwrite only these skills' `SKILL.md`.

### Project-local installation

For skills that travel with one project instead of being globally available, replace the home-directory root with:

```text
.agents/skills
.pi/skills
.opencode/skills
.claude/skills
```

Review remote instructions before installing them in an environment where the agent can modify files or run commands.

## What they cover

- **Authoring**: menu-action plugins (Lua in TOML) and lessfilter rule tables — the two user-facing extension points of f:ist.
- **Operating**: launching panes non-interactively with the right filters/options, and driving the interactive interface (queue, copy/rename flows, pagers, metadata).
- **Source of truth**: every skill points at the shipped config assets (`assets/config/*.toml`, `assets/actions/*.toml`) and the relevant subcommand help rather than duplicating them.

## References

- [F:ist repository](https://github.com/Squirreljetpack/fist)
- Shipped config defaults: [`assets/config/`](../config/) (`config.toml`, `mm.toml`, `lessfilter.toml`, `pager.toml`, `actions.toml`)
- Shipped menu actions: [`assets/actions/`](../actions/)
- [Matchmaker](https://github.com/Squirreljetpack/matchmaker) (the picker layer; bind/action semantics)
- [Codex skills](https://developers.openai.com/codex/build-skills) · [pi skills](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/skills.md) · [OpenCode skills](https://opencode.ai/docs/skills) · [Claude Code skills](https://code.claude.com/docs/en/skills)
