# Machines and browsers

Where a Castle script runs, what the machine gets, which browser drives the site, and what
to do when a run never gets going.

- [Container or virtual machine](#container-or-virtual-machine)
- [What the first run installs](#what-the-first-run-installs)
- [Kept or temporary](#kept-or-temporary)
- [The browser](#the-browser)
- [Firefox, Google Chrome, and other browser images](#firefox-google-chrome-and-other-browser-images)
- [A Dockerfile of your own](#a-dockerfile-of-your-own)
- [How a run reaches the box](#how-a-run-reaches-the-box)
- [When a run never gets going](#when-a-run-never-gets-going)

## Container or virtual machine

Every automation says where it runs: **Runs in** (container or virtual machine) and **Runs
on** (the system). Castle scripts run on Linux systems only.

| | Container (Docker) | Virtual machine (QEMU) |
|---|---|---|
| Starts in | seconds | a minute or more (a whole computer boots) |
| Memory | what its programs use, up to the automation's *Memory (GB)* | the whole *Memory (GB)*, from the moment it boots |
| Needs | Docker Desktop running | QEMU on the box |
| Its screen | in memory (Xvfb), seen through the bot's screen in Clones | a window of its own |
| A person answers a check by | **Take control** in Clones | the VM's window |
| Good for | browser bots, many at once | apps that need a full desktop, or when Docker is not there |

**A container is the default for scripts, and the better choice almost always.** The
Walmart example runs in the **Debian 13 container**. Pick a VM when Docker is not installed
or will not start.

Which container system:

| System | For scripts |
|---|---|
| **Debian 13** | The one to use. Its own `chromium` package works in a container. |
| Ubuntu 24.04 | Ubuntu ships its browsers as snaps, which a container cannot run. Pick Debian. |
| Arch Linux | `chromium` from pacman. |
| Fedora 45 | Set up from the apt names, which mostly match. |

Each bot has a machine of its own. Two bots of one automation never share a browser, cookies
or a sign-in.

## What the first run installs

On every start Castle writes the script's project fresh on the box
(`app/data/clonewars/automations/<automation>-<bot>/`) and puts it on the machine:

| File | What it is |
|---|---|
| `automation.castle` | your script, as saved |
| `main.py` | the Python it compiles to |
| `castle/` | the library: the run and its reports, the browser, what is done to rows |
| `castle-run.json` | this run's ticket: the box's addresses and a secret that lets this run, and only this run, report |

Then the machine's setup installs its packages, and `castle/run.sh` runs the script as the
desktop user, on the machine's screen:

1. **Packages.** `python3-venv chromium fonts-liberation` on Debian (`python chromium
   ttf-liberation` on Arch), plus **More packages to install** from the automation's editor.
   A script with no `website` only gets Python.
2. **A Python environment** in `~/.castle/venv`.
3. **Playwright** into it, the library that drives the browser (scripts with a `website`).
4. If the system has no Chromium, **Playwright's own Chromium**.

That is a few minutes the first time and nothing after, on a kept machine. The machine's own
log is `~/castle-automation.log`.

**More packages to install** takes package names separated by spaces, as the system's package
manager knows them (`jq`, `ffmpeg`, `tesseract-ocr`). They are installed on the first run
and kept. Python libraries for a `python:` block are not packages; Debian packages many as
`python3-<name>`, but the script's environment does not see the system's Python libraries.
For those, install them from the block itself:

```text
python:
    import subprocess, sys
    subprocess.run([sys.executable, "-m", "pip", "install", "--quiet", "openpyxl"], check=True)
```

## Kept or temporary

A machine is **kept** unless the automation's rules say *Temporary*. A kept container's disk
is its writable layer; a kept VM has its own disk. Either way, what the first run installed
and the browser's profile stay for the next run.

A **temporary** machine starts clean every run and forgets everything when it stops. It
reinstalls Python, Chromium and Playwright every time (minutes), and every site sees a brand
new browser. Use it when a run must not remember anything; otherwise keep the machine.

A kept container is made again, empty, when Castle's container image changes (a new Castle
version can do this). The next run installs everything again, by itself.

## The browser

Scripts drive **Chromium** through **Playwright**:

- It is a real browser with a window, on the machine's screen, so the bot's screen in Clones
  shows what it is doing and you can take over at any time.
- Its **profile** is kept in `~/.castle/browser` on the machine: cookies, sign-ins, what a
  site remembers about it. Yesterday's browser comes back today.
- It uses the system's Chromium package (Debian's `chromium`). If that will not start,
  Castle gets Playwright's own Chromium and says so in the log.
- In a container it starts with `--no-sandbox`: a container has no room for Chromium's own
  sandbox, and the container is the sandbox.
- The page is in US English (`en-US`) and the window fills the screen (1440x900 in a
  container, unless the automation's *Screen* says otherwise).

## Firefox, Google Chrome, and other browser images

**Chrome.** Chromium is Chrome's open-source core: the same engine, the same pages, the same
developer tools you use to find selectors. Sites see a Chrome-family browser. Google's own
Chrome build is not in Debian's packages, and a script cannot pick it.

**Firefox.** Not today. Playwright cannot drive the Firefox a Linux system ships; it drives
a Firefox it builds itself, and the language has no line for choosing a browser yet. Debian's
own Firefox (`firefox-esr`) can still be installed and driven by a project automation, the
way Penguin AutoSurf does.

**Ready-made browser containers** (`selenium/standalone-chrome`, `linuxserver/firefox`,
`browserless/chrome` and the like) are not what Castle runs. A script's container is made from
Castle's own image, `castle/automation-<system>`, because that image is what gives a bot its
screen in Clones, **Take control**, the setup that installs packages, and the stop when the
run is done. A container from another image would have none of them.

## A Dockerfile of your own

A script cannot carry a Dockerfile. The script says what the bot *does*; the automation's
settings say *where* it runs and what extra packages it gets; Castle's image is the same for
every bot. That keeps every script runnable on any Castle box, and every bot's machine
something Castle knows how to start, show, hand to a person and stop.

What to use instead:

| You want | Use |
|---|---|
| a program from the system's packages | **More packages to install** |
| a Python library | `pip install` from a `python:` block (see above) |
| a whole program of your own | a **project** automation instead of a script: it brings its own folder or git repository and runs its own commands |

**Changing Castle's image** is for Castle's own development: the images are
`app/clonewars/docker/automation/Dockerfile.<system>`, built by `container.ps1` and tagged by
a hash of that folder. An edit there rebuilds the image on the next start and remakes every
kept container of that system, for every automation, empty. Keep what goes in there to what
every bot needs.

## How a run reaches the box

As it runs, the script reports every step, row, value and picture to the Castle box over
HTTP, using the secret in `castle-run.json`. The ticket lists the box at the addresses a
machine can see it at, tried in order:

| From | The box is |
|---|---|
| a container | `host.docker.internal`, then `172.17.0.1`, then `10.0.2.2` |
| a VM | `10.0.2.2` (QEMU's address for the computer it runs on), then `host.docker.internal` |

The port is the box's own. The secret lets the run report to its own results and nothing
else, and it stops working when the run ends. If the box cannot be reached, the reports are
kept on the machine in `~/castle-results.ndjson` and the run carries on.

## When a run never gets going

| You see | Look at |
|---|---|
| The run stays **STARTING** | The machine is being set up: the first run installs a browser (minutes). The bot's screen and the machine's log show where it is. |
| **Docker Desktop** will not start, or fails with *out of memory* | Close what is using memory, then start Docker Desktop again. Its own log is `%LOCALAPPDATA%\Docker\log\host\com.docker.backend.exe.log`. Or switch the automation to a VM. |
| `getting the machine ready` failed | The machine could not install packages or Playwright, nearly always because it is offline. |
| `Castle scripts run on Linux machines` | The automation is on an Android system. Pick a Linux one. |
| The run starts, the browser opens, but Clones shows nothing | The machine cannot reach the box. Is the box still running on the same port? Look for `~/castle-results.ndjson` on the machine. |
| `this bot has no <field> yet` | Fill in the bot's fields and save. |
