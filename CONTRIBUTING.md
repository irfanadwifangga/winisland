# Contributing to winisland

Thanks for your interest. The project is small and personal-scale, so a little coordination goes a long way.

## Before you start

- For anything bigger than a small bug fix, open an issue first so we can agree on the approach.
- Keep pull requests small and focused on one change.

## Setup and checks

```powershell
.\setup.ps1 -Check   # verifies the toolchain and runs cargo check
cargo fmt
cargo clippy
```

## Project rules

1. **Memory matters.** This project exists to stay small. Measure the *private working set* before and after
   your change (Task Manager > Details tab > add the column "Memory (private working set)") and report both
   numbers in the pull request. There is no hard budget, but unexplained jumps will be questioned.
2. **The renderer is locked to Slint's software renderer** (set in code through `BackendSelector`), and the
   Slint feature list in `Cargo.toml` is intentionally trimmed. Do not re-add another renderer without
   measurements. Effects the software renderer does not support (for example `drop-shadow-*`) will not render.
3. **Blocking Windows API calls** (for example WinRT `.join()`) must run on a background thread, never on the UI thread.
4. **Window handling:** the overlay style and position are applied from inside the event loop (`try_apply`, `place`).
   In `ui/main.slint`, the `TouchArea` must keep covering the whole window, otherwise hover breaks in the gap
   around the floating pill.
5. **Only claim what you verified.** Say what you ran and what you saw. If something is untested, say so.

## Commit messages

Use a short prefix (`feat:`, `fix:`, `build:`, `docs:`) and a summary line, followed by bullet points.
Put what you verified and anything you did not verify in the body.

## Language

UI text is in English. Code comments are currently mostly in Indonesian; English comments are welcome in new code.

## License of contributions

By submitting a contribution you agree that it is licensed under the [MIT License](LICENSE), the same as the
rest of the project. Slint is a separate dependency under its own license (see the README).
