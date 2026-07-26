# Pomotui Dashboard prototype verdict

The throwaway Ratatui prototype asked which Dashboard hierarchy remains useful
across wide and narrow terminals while exposing the Current Session, Current
Task, Focus Cycle, next Pending Session, today's statistics, recent Session
History, commands, settings, and mouse targets without clutter.

## Decision

Use the **Timer First** direction for the Dashboard:

- Wide terminals place Tasks and Today side by side across the top, with a
  large full-width Current Session countdown below. This prevents Task titles
  and accumulated time from being truncated and gives the countdown room to
  breathe.
- Narrow terminals stack Tasks over the countdown. Today's statistics and
  recent Session History become separate views instead of compressed panels.
- Keep Current Task, attributed focus time, Focus Cycle position, and the next
  Pending Session close to the countdown.
- Use the Timeline direction's explicit past/current/next hierarchy for a
  separate Session History view, not the default Dashboard.
- Settings becomes a full-screen view on narrow terminals.
- Paused and Pending Sessions share warm gold but require explicit labels.
  Break Sessions use a derived green token. Focus and primary actions use the
  signature Vermilion token.
- Today includes a compact seven-day trend and its numeric average.
- Support `j/k` for moving between Tasks and `h/l` for switching views, with
  arrow keys as equivalent inputs. Skip uses uppercase `K` or the command
  palette so it does not conflict with navigation.
- After a Session completes, briefly show a building-collapse character
  animation and then automatically reveal the next Pending Session. The
  animation is a Timer Frontend effect; the Pending Session never starts
  automatically.

## Primary source

The complete disposable prototype, deterministic fixtures, findings, and
wide/narrow SVG snapshots are preserved outside `main`:

- Branch: `prototype/pomotui-dashboard`
- Initial commit: `9444604` (`prototype: explore Pomotui dashboard layouts`)
- Feedback revision: `fd01ff7`
- Navigation and completion transition: `f05828c`
