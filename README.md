<p align="center">
  <a href="https://pommersche92.github.io/durin/"><img src="icon.png" alt="Durin Icon" width="160" /></a>
</p>

# Durin

> "Durin who was called the Deathless."
>
> *J.R.R. Tolkien, The Lord of the Rings (Appendix A)*

Durin is named after the legendary Dwarf-king of Khazad-dum, a symbol of depth, endurance, and mastery beneath the surface.

**Durin - The ultimate deep delver into heap layouts.**

Today, Durin already provides live, profile-based RAM tracking with an overlay, global toggle, and tray control. Tomorrow, it goes deeper: heap layout and allocation-path inspection for selected processes.

---

## Table of Contents

- [Why Durin](#why-durin)
- [Current Feature Set](#current-feature-set)
- [Platform Support](#platform-support)
- [Architecture Overview](#architecture-overview)
- [Getting Started](#getting-started)
- [Usage](#usage)
- [Profiles and settings.toml](#profiles-and-settingstoml)
- [Screenshot Placeholders](#screenshot-placeholders)
- [Roadmap](#roadmap)
- [Changelog](#changelog)
- [Contributing](#contributing)
- [License](#license)

---

## Why Durin

Observability tools often stop at process-wide memory counters. Durin is built around the idea that practical diagnostics need a smooth path from:

1. **What is growing right now?** (live RAM trend)
2. **Which process group is responsible?** (profile/group model)
3. **What objects and allocation paths caused the growth?** (planned heap inspection)

Durin starts with low-friction, real-time RAM tracking and is intentionally architected to evolve into deep heap introspection without forcing a redesign.

---

## Current Feature Set

### Live Overlay

- Always-on-top desktop overlay window.
- Real-time RAM graph per configured process group.
- Lightweight polling with bounded in-memory sample history.

### Global Controls

- Global keyboard shortcut to toggle overlay visibility.
- System tray icon and menu support:
  - Toggle overlay
  - Quit application

### Profile-Based Tracking

- Multiple savable profiles in `settings.toml`.
- Profiles include:
  - Distinct profile name
  - Description
  - One or more process groups
- Group targets can be:
  - Running process selection (PID-based)
  - Manual process-name target (for currently not-running processes)

### Profile CRUD via UI

- Create profile
- Edit profile
- Delete profile
- Add/remove groups
- Add/remove targets in each group

### Future-Ready Heap Layer

- Stable trait abstraction for heap inspection backends.
- Stub backend already integrated in UI and app architecture.
- Clear extension points for Linux and Windows native implementations.

---

## Platform Support

### Supported Targets

- **Windows**
- **Linux** (broad distro coverage; behavior can vary by desktop/session)

### Notes

- Global hotkey/tray behavior on Linux may depend on session type (X11 vs Wayland) and desktop environment capabilities.
- Heap inspection is not implemented yet.

---

## Architecture Overview

Durin is organized to keep runtime orchestration, domain logic, and UI modular.

```mermaid
flowchart TD
    A[main.rs] --> B[app.rs - runtime orchestration]
    B --> C[tracking.rs - RAM sampling engine]
    B --> D[process.rs - running process discovery]
    B --> E[config.rs - settings/profile persistence]
    B --> F[heap.rs - heap backend abstraction]
    B --> G[ui/pages - page renderers]
    G --> H[ui/controls - reusable widgets]
```

### Key Design Principles

- Keep UI pages in separate modules (`src/ui/pages`).
- Keep reusable controls in separate modules (`src/ui/controls`).
- Keep persistence centralized (`config.rs`).
- Keep future heap feature behind a backend trait (`heap.rs`).

---

## Getting Started

### Prerequisites

- Rust toolchain (stable)
- Cargo
- On Linux, typical desktop dependencies required by `eframe` and tray/hotkey crates

### Clone and Build

```bash
git clone <your-repo-url>
cd durin
cargo check
cargo build
```

### Run (development)

```bash
cargo run
```

### Build optimized release

```bash
cargo build --release
```

---

## Usage

### Quick Start

1. Launch Durin.
2. Create a new profile in the left panel.
3. Enter profile name and description.
4. Add one or more groups.
5. In each group, add processes:
   - select currently running process entries
   - or add manual process names for future matches
6. Save profile.
7. Watch live RAM graph updates in the center panel.

### Toggle Overlay

Default hotkey:

- `Ctrl+Shift+R`

Alternative controls:

- Tray icon click
- Tray menu action

### Typical Monitoring Scenarios

#### Scenario A: Browser + Tooling

- Group `Browser`: `chrome`, `firefox`, or selected browser PID
- Group `Dev Tools`: `code`, `rust-analyzer`, build processes

#### Scenario B: Service + Worker Stack

- Group `API`: process name of server binary
- Group `Workers`: worker process names, queue runners

#### Scenario C: Regression Tracking Session

- Profile `Leak Repro 2026-08-11`
- Description includes testcase and expected baseline
- Capture memory trend while reproducing workload

---

## Profiles and settings.toml

Durin persists profile and UI state in `settings.toml` at project runtime path.

### Minimal Example

```toml
overlay_visible = true
hotkey = "Ctrl+Shift+R"
active_profile = "Daily Monitoring"

[[profiles]]
name = "Daily Monitoring"
description = "Track browser and IDE memory during development"

[[profiles.groups]]
name = "Browser"

[[profiles.groups.targets]]
display_name = "chrome (Name Match)"
process_name = "chrome"
pid = nil
manual = true

[[profiles.groups]]
name = "IDE"

[[profiles.groups.targets]]
display_name = "Code.exe (PID 12345)"
process_name = "Code.exe"
pid = 12345
manual = false
```

### Recommended Profile Naming Convention

- `context-purpose-date`
- Examples:
  - `frontend-loadtest-2026-08-11`
  - `service-leak-repro-v2`
  - `daily-dev-observability`

---

## Screenshot Placeholders

Add real screenshots into `screenshots/` and keep these links unchanged.

### Overlay Overview

![Overlay Overview Placeholder](screenshots/overlay-overview.png)

### Profile Editor

![Profile Editor Placeholder](screenshots/profile-editor.png)

### Group and Process Assignment

![Grouping Page Placeholder](screenshots/grouping-page.png)

### Live RAM Graph

![RAM Graph Placeholder](screenshots/ram-graph.png)

### Tray Menu and Toggle

![Tray Menu Placeholder](screenshots/tray-menu.png)

---

## Roadmap

Durin is being developed in layers from broad observability to deep memory diagnostics.

### Near Term

- [ ] Better graph UX (zoom range, smoothing toggles, per-group visibility)
- [ ] Configurable sampling interval and retention window
- [ ] Export snapshots to CSV/JSON
- [ ] Enhanced profile validation and duplicate target warnings

### Heap Inspection Milestone

- [ ] Implement real heap backend selection by platform
- [ ] Add process attach/permission diagnostics
- [ ] Capture heap summary snapshots per tracked target
- [ ] Visualize heap region composition in overlay
- [ ] Correlate RAM graph spikes with heap snapshot timelines
- [ ] Allocation-site attribution and leak candidate hints

### Longer Term

- [ ] Historical session storage and comparison
- [ ] Time-aligned event annotations in graph
- [ ] Plugin/backend interface for runtime-specific analyzers
- [ ] Optional remote target support (advanced)

---

## Changelog

### [0.1.0] - 2026-08-11

Initial public baseline.

Added:

- Modular UI architecture under `src/ui/pages` and `src/ui/controls`
- Overlay with live RAM graphing
- Global hotkey visibility toggle
- Tray icon/menu controls
- Profile management with create/edit/delete
- Group-based process target configuration
- Running process selection and manual target entry
- Persistent `settings.toml` storage model
- Heap inspection trait abstraction and explicit stub backend
- Extensive rustdoc comments across modules and functions

Known limitations:

- Heap inspection not yet implemented
- Linux tray/hotkey behavior depends on environment capabilities

---

## Contributing

Contributions are welcome.

Recommended workflow:

1. Open an issue describing your proposal.
2. Fork and create a focused feature branch.
3. Keep changes modular and include tests where practical.
4. Run:

```bash
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

5. Submit a pull request with:
   - motivation
   - implementation notes
   - validation steps
   - screenshot updates (if UI changes)

---

## License

Durin is licensed under **GPL-3.0-only**.

See project metadata in `Cargo.toml` for current canonical license declaration.

---

## Name and Vision Recap

Durin is a memory observability tool that begins with clear live RAM insight and grows toward deep heap understanding.

If memory profiling tools are mountain maps, Durin is the delver that keeps going when the tunnels descend.
