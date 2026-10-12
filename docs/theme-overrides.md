# Theme overrides

A theme (see the [TUI guide](tui.md#themes)) sets every colour the UI uses.
`config.toml` can set any of those hues over whichever theme is chosen, with
the same keys a theme file uses:

```toml
# config.toml
[theme]
accent = "#ff8800"
running = "#ffd166"
```

An override is still made readable against the terminal background and reduced
to the terminal's colour depth; it is the one thing that changes colours on a
16-colour terminal, and `mono` ignores it. An unknown key or a value that is
not `#RRGGBB` stops the TUI at startup with an error naming `config.toml`.

## Older token overrides

Earlier versions accepted per-token overrides such as
`[theme.tokens.accent] fg = "#00bfff"`. Their `fg` colours still apply, mapped
onto the hue that now paints the same thing:

| Token | Hue |
| --- | --- |
| `accent`, `running`, `waiting`, `blocked`, `heading`, `link`, `diff_add`, `diff_remove` | the same name |
| `completed` | `success` |
| `failed` | `failure` |
| `approval` | `attention` |
| `person` | `user` |
| `inline_code` | `code` |

`[theme] <hue>` wins when both set the same hue. The other tokens (`ground`,
`text`, `secondary`, `dim`, `border`, `border_focused`, `replayed`,
`cancelled`, `thinking`, `code_block`), and `bg` and `modifiers` on any token,
change nothing: those colours come from the terminal, and the TUI says which
entries it ignored when it starts. A malformed `[theme.tokens.accent] fg` stops
the TUI with an error naming `config.toml`.

Aether reads `config.toml` without rewriting it. Choices made in `/theme` and
`/settings` are saved separately in `ui.toml`.
