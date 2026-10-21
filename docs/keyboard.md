# Keyboard guide

[العربية](keyboard.ar.md)

Tab and Shift+Tab navigate the native controls. Enter or Space activates a
button; Space changes a checkbox. In recording mode, the primary control stays
in place as its action changes between Start and Stop. Pause changes to Resume
in place. If a focused pause/resume control disappears on stopping, focus moves
to the primary control. Inapplicable controls are absent.

| Shortcut     | Action                    |
| ------------ | ------------------------- |
| Ctrl+Shift+R | Start or stop recording   |
| Ctrl+Shift+P | Pause or resume recording |
| Ctrl+Shift+V | Open and refresh Videos   |
| Ctrl+Shift+S | Open Settings             |
| Ctrl+Shift+N | Open another window       |

Shortcuts work inside TakeDock, even while a file job is running. They are ignored
inside text/select inputs, during composition, and on repeated keydown events.
Ctrl+Shift+N also works inside inputs; it does not trigger a camera command.
Screen reader users may need their ordinary pass-through command if a reader
intercepts a shortcut. Camera actions require Open Camera active in video mode.

Videos use native checkboxes and a table. Copy, move, and delete accept multiple
selected completed files; rename appears only for one. Delete presents one batch
confirmation. Rename preserves the extension. Unfinished/unrecognized containers
are listed without selection controls. Results and per-file details are ordinary
text for navigation, with no live announcements or automatic result speech.

The Activity section contains a labelled native progress bar. Screen readers
can announce/beep percentage changes using their own progress settings. Transfer
percentage applies to the named current file; batch counts and verification
remain available as ordinary text. Name each window in Settings; the title is
used in Alt+Tab. Windows share the phone, folder and preferences.

The ready-to-speak cue is enabled by default. After a new Start, wait for its
short PC tone to finish before speaking. **Preparing to speak** and **Ready to
speak** are ordinary text and do not move keyboard focus or announce themselves.
Settings controls the cue independently and saves its post-confirmation guard;
changes apply to the next Start. Stop or Pause cancels a pending cue; Resume and
another window do not replay it. The preparation preamble remains in the video.

Optional success feedback is silent by default. Settings can enable its rising
two-note sound. A Start using the readiness cue suppresses its generic success
feedback so the distinct PC cue remains the signal to begin speaking.
Failures use a falling two-note sound. A verification timeout indicates an
unconfirmed command: inspect the phone before reconnecting or taking another
action. TakeDock never automatically retries a camera toggle.

For an eligible take, Tab to **Delete this take**, press Space, then activate
the same **Stop and delete** button with Enter/Space. Selecting the checkbox
alone performs no action. Selection belongs to this window and resets for a new
take, connection or reopened panel. Ctrl+Shift+R always starts/stops normally,
including while the checkbox is selected. Progress/results are ordinary text
with no automatic announcements. Missing/split/ambiguous file evidence leaves
ordinary Stop available; use explicit selection and confirmation in Videos.
An unknown deletion result requires inspecting Videos before further action.

A clean file success remains in the main result area and Activity for ten
seconds, measured once for all windows. Failures, cancellations, retained or
unknown outcomes and warnings remain until **Hide result**. Hiding acknowledges
that result across windows and never cancels work. Active jobs have separate
**Cancel** buttons. **Operation history** is a native disclosure, closed when
each window opens; it retains per-file details for this running process. While
open, its displayed records stay until you close it, even as newer operations
arrive. Closing the application clears this process-local history.

If focus is inside an expired/hidden result, that window keeps the card until
focus leaves. Passive expiry and hiding in another window do not move focus.
An explicit local Hide may move focus to the next Hide button or history summary
only while its button still owns focus. A confirmed rename updates the selected
checkbox's name and current identity without replacing its native control.
