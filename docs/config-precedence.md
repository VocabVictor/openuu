# Where a setting's value comes from

Five layers. The first one that has the key wins.

| # | Layer | Set by | Written to disk? |
| --- | --- | --- | --- |
| 1 | Environment | `OPENUU_OPT_<KEY>=value` | no |
| 2 | Command line | `--option <key>=<value>` | no |
| 3 | Deployment lock | `openuu-config.json` with the key in `locked` | yes, in the provisioning file |
| 4 | The user's own setting | the app's settings pages | yes, in the config files |
| 5 | Deployment default | `openuu-config.json`, key not locked | yes, in the provisioning file |
| 6 | Built-in default | `libs/base/src/config/builtin.rs` | no, baked into the binary |

Layers 1 and 2 live in `libs/base/src/config/overrides.rs`; 3 and 5 in
`config/provision/`; 6 in `config/builtin.rs`. Layer 4 is whatever the app
itself wrote when someone changed a setting.

Underneath all of it, `hbb_common` has its own defaults for keys nobody here
sets. Those are upstream's and this repository does not edit them; where the
product wants a different one, it goes in layer 6 — see
`builtin::apply_ui_defaults` for the worked example.

## The two at the top

They exist to answer one question without editing anything: *what does this
build do if I change this one value*.

```sh
# one run, one setting
OPENUU_OPT_ALLOW_D3D_RENDER=N openuu

# same thing on the command line, repeatable
openuu --option allow-d3d-render=N --option disable-udp=N
```

The variable name is the key shouted: upper case, `-` becomes `_`. It is only
ever built in that direction — `allow-d3d-render` and `allow_d3d_render` would
produce the same variable, so the environment is read by asking each known key
whether its variable is set, never by turning a variable name back into a key.
A variable that looks like ours but matches no key is ignored without a word,
because it is far more likely to be someone else's than a typo of ours.

Nothing they set is written anywhere. The next start without them is the old
behaviour, and no settings page will ever show the overridden value as though
the user had chosen it.

Every override is logged, with its value:

```
event=config_override key=allow-d3d-render value=N source=env
```

## They beat a lock, and that is deliberate

A provisioned `locked` key is layer 3, above the user's own setting — but below
these two. A lock is a deployment's policy and this lets anyone with a shell on
the machine step over it.

That is the honest reading of what a lock can do. It is a fence in the UI, not a
security boundary: whoever can set an environment variable for the process can
also edit the configuration file it reads, or run a build of their own. Putting
the override below the lock would not stop that person; it would only make
diagnosing a locked deployment harder for the people entitled to do it.

## What they cannot reach

* **Anything read before they are applied.** They are applied in `core_main`
  after the built-in defaults, the provisioning file and the custom client
  file — so `global_init` and provisioning itself see the stored values.
* **A secret.** `permanent-password` and friends have their own stores; an
  override that set one would also be the easiest way to leave a password in a
  shell history or a process listing. Refused, and the refusal is logged.
* **A key that is in none of the tables.** `KEYS_SETTINGS`,
  `KEYS_DISPLAY_SETTINGS` and `KEYS_LOCAL_SETTINGS` are what can be overridden,
  because they are what the layers below can carry. An unknown key has no table
  to be written into; it is logged and skipped.
* **A child process, for `--option`.** Environment variables are inherited, so
  they reach the server process and the session windows. Command-line options
  are not passed on.

## Diagnostics are not settings

`RUSTDESK_QOS_VERBOSE` and the other `RUSTDESK_*` variables switch logging and
test fixtures on. They are read where they are used, are not keys, and have
nothing to do with the table above. Do not add product behaviour behind one:
a setting that decides what the application *does* belongs in the tables, where
it can be provisioned, locked, and seen in the UI.

## Changing a default

Change it in layer 6 and it reaches every install that has never set the key.
It does **not** reach a peer or a config file that already has the key written
in it — that is what `view_style_migration.rs` had to be written for, once, and
the note in that file says why there should not be a second one.
