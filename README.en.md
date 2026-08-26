# Pause (休一下)

English | [简体中文](README.md)

A native macOS menu-bar break reminder. It stays out of your way until it's time — then a beautiful full-screen nature photo invites you to step away from the screen, look into the distance, and stretch.

## Screenshots

| Reminder | Settings |
| :---: | :---: |
| ![Reminder](docs/reminder-en.png) | ![Settings](docs/settings-en.png) |

When a reminder appears, the Start Break button carries the auto-start countdown (e.g. "Start Break (8s)"); when it reaches zero the break begins automatically. You can still delay or start it manually at any time.

Built with **Swift + SwiftUI + a little AppKit, MVVM architecture, zero third-party dependencies**. Requires macOS 13+, native Apple Silicon build. Current version **v1.0.0**.

## Getting Started

```bash
# Option 1: run from source (note: a bare binary uses a different defaults domain than the .app)
swift run

# Option 2: build the release .app (Release + ad-hoc signed + icon)
./build.sh
open build/Pause.app
```

- The menu bar always shows the **time until your next break** (e.g. `43m`); the app never occupies the Dock (LSUIElement).
- During a break the menu bar shows minutes left; when paused it shows `⏸`; when a reminder is waiting it shows `!`.
- Menu: **next-break countdown / Break Now / Pause Reminders / Settings… / Quit**.

## Features

- Reminder intervals of 30/45/60 min + custom (10–180 min)
- **Real-usage timing** (on by default): the timer only counts while you're actively using the Mac — away time, display sleep, and system sleep are excluded, so you're reminded only after real usage fills the interval
- **Auto-start break** (on by default): once a reminder appears, the Start Break button carries a per-second countdown (10/20/30/60 s); doing nothing automatically starts the break
- Delay break 1–5 min + custom (1–15 min), unlimited times
- Custom break duration (1–30 min)
- Full-bleed wallpaper reminder window (900×600 auto-scaled, 16 pt rounded corners, slow Ken Burns motion, crossfade)
- Wallpaper pipeline: background prefetch + local cache (max 18 images, auto-eviction, ≤3200 px downsampling) + runtime-generated fallback when offline
- Manual wallpaper switching with preview
- Break countdown page + End Early
- Pause / resume reminders, break now
- Launch at login (SMAppService)
- Reminder window opacity (30%–100%, live) and overlay-other-windows option
- English / 简体中文 UI, switchable instantly
- Multi-display: the reminder appears centered on the display your pointer is on
- Smart avoidance: reminders due while the screen is locked / screensaver / display sleep are deferred and shown right after unlock

## Architecture

Unidirectional data flow `View → ViewModel → Service → Model/system`. Views only observe state and send intents; services own timing, networking, caching, and system integration. `ReminderService` is the single source of truth for time. All phases live in an explicit state machine (`working → reminding → breaking`, `reminding → snoozing`, any → `paused`) with an injected clock, fully unit-testable.

See the [Chinese README](README.md) for an in-depth explanation of every subsystem (state machine, idle handling, wallpaper pipeline, multi-display, etc.).

## Project Layout

```
Pause/
├── App/          # @main, app state, localization (builtin zh/en tables), DI container
├── Models/       # settings, state machine, break session, wallpaper item
├── ViewModels/   # reminder / break / settings / menu-bar view models
├── Views/        # reminder page, break page, settings, menu, wallpaper backdrop
├── Services/     # settings store, reminder timing, wallpaper fetch/cache, activity, login item
└── Windows/      # borderless non-activating NSPanel, fade in/out
Tests/PauseTests/ # 30 unit tests
```

## Testing & Release Build

```bash
swift test    # 30 cases: state machine / idle deferral / auto break / cache eviction / scaling / URL validation
./build.sh    # produces build/Pause.app (Release, arm64, ad-hoc signed)
```

## Known Limitations

- No topic filtering for the online image source; wallpapers come from the network (default: picsum.photos random photos).
- Reminders still appear during other apps' fullscreen presentations (the window never steals focus; lock screen / screensaver / display sleep are avoided).
- `swift run` and `build/Pause.app` use different UserDefaults domains; use the .app for daily use.
- Ad-hoc signature runs only on the build machine; redistribution requires re-signing with a Developer ID.

## Scope

No accounts, cloud sync, task management, check-ins, complex statistics, community, subscriptions, ads, or plugin systems — on purpose.

## Acknowledgement

This project's code and documentation were generated with the **[GLM large language model by Z.ai (Zhipu AI)](https://z.ai)** via the ZCode coding agent, which also drove the design, implementation, testing, and release.

## License

[MIT](LICENSE) © 2026 Vindac
