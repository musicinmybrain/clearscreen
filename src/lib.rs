//! Cross-platform ter inner: (), input_flags: (), output_flags: (), control_flags: (), local_flags: (), control_chars: ()minal screen clearing.
//!
//! This library provides a set of ways to clear a screen, plus a “best effort” convenience function
//! to do the right thing most of the time.
//!
//! Unlike many cross-platform libraries, this one exposes every available choice all the time, and
//! only the convenience function varies based on compilation target or environmental factors.
//!
//! 90% of the time, you’ll want to use the convenience short-hand:
//!
//! ```
//! clearscreen::clear().expect("failed to clear screen");
//! ```
//!
//! For anything else, refer to the [`ClearScreen`] enum.

#![doc(html_favicon_url = "https://raw.githubusercontent.com/watchexec/clearscreen/main/logo.png")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/watchexec/clearscreen/main/logo.png")]
#![warn(missing_docs)]

use std::{
	io::{self, Write},
	process::{Command, ExitStatus},
};

use terminfo::{capability, expand::Context, Database};
use thiserror::Error;

/// Ways to clear the screen.
///
/// There isn’t a single way to clear the (terminal/console) screen. Not only are there several
/// techniques to achieve the outcome, there are differences in the way terminal emulators intepret
/// some of these techniques, as well as platform particularities.
///
/// In addition, there are other conditions a screen can be in that might be beneficial to reset,
/// such as when a TUI application crashes and leaves the terminal in a less than useful state.
///
/// Finally, a terminal may have scrollback, and this can be kept as-is or cleared as well.
///
/// Your application may need one particular clearing method, or it might offer several options to
/// the user, such as “hard” and “soft” clearing. This library makes no assumption and no judgement
/// on what is considered hard, soft, or something else: that is your responsibility to determine in
/// your context.
///
/// For most cases, you should use [`ClearScreen::default()`] to select the most appropriate method.
///
/// In any event, once a way is selected, call [`clear()`][ClearScreen::clear()] to apply it.
///
/// # Example
///
/// ```
/// # use clearscreen::ClearScreen;
/// ClearScreen::default().clear().expect("failed to clear the screen");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClearScreen {
	/// Does both [`TerminfoScreen`][ClearScreen::TerminfoScreen] and
	/// [`TerminfoScrollback`][ClearScreen::TerminfoScrollback], in this order, but skips the
	/// scrollback reset if the capability isn’t available.
	///
	/// This is essentially what the [`clear`] command on unix does.
	/// [`clear`]: https://invisible-island.net/ncurses/man/clear.1.html
	Terminfo,

	/// Looks up the `clear` capability in the terminfo (from the TERM env var), and applies it.
	///
	/// A non-hashed terminfo database is required (this is a [terminfo crate] limitation), such as
	/// the one provided with ncurses.
	///
	/// [terminfo crate]: https://lib.rs/crates/terminfo
	TerminfoScreen,

	/// Looks up the `E3` (Erase Scrollback) capability in the terminfo (from the TERM env var), and applies it.
	///
	/// The same terminfo limitation applies as for [`TerminfoScreen`][ClearScreen::TerminfoScreen].
	TerminfoScrollback,

	/// Performs a terminfo-driven terminal reset sequence.
	///
	/// This prints whichever are available of the **rs1**, **rs2**, **rs3**, and **rf** sequences.
	/// If none of these are available, it prints whichever are available of the **is1**, **is2**,
	/// **is3**, and **if** sequences. If none are available, an error is returned.
	///
	/// This generally issues at least an `ESC c` sequence, which resets all terminal state to
	/// default values, and then may issue more sequences to reset other things or enforce a
	/// particular kind of state. See [`XtermReset`][ClearScreen::XtermReset] for a description of
	/// what XTerm does, as an example.
	///
	/// Note that this is _not_ analogous to what `tput reset` does: to emulate that, issuing first
	/// one of VtCooked/VtWellDone/WindowsCooked followed by this variant will come close.
	///
	/// The same terminfo limitation applies as for [`TerminfoScreen`][ClearScreen::TerminfoScreen].
	TerminfoReset,

	/// Prints clear screen and scrollback sequence as if TERM=xterm.
	///
	/// This does not look up the correct sequence in the terminfo database, but rather prints:
	///
	/// - `CSI H` (Cursor Position 0,0), which sets the cursor position to 0,0.
	/// - `CSI 2J` (Erase Screen), which erases the whole screen.
	/// - `CSI 3J` (Erase Scrollback), which erases the scrollback (xterm extension).
	XtermClear,

	/// Prints the terminal reset sequence as if TERM=xterm.
	///
	/// This does not look up the correct sequence in the terminfo database, but rather prints:
	///
	/// - `ESC c` (Reset to Initial State), which nominally resets all terminal state to initial
	///   values, but see the documentation for [`VtRis`][ClearScreen::VtRis].
	/// - `CSI !p` (Soft Terminal Reset), which nominally does the same thing as RIS, but without
	///   disconnecting the terminal data lines… which matters when you’re living in 1970.
	/// - `CSI ?3l` (Reset to 80 Columns), which resets the terminal width to 80 columns, or more
	///   accurately, resets the option that selects 132 column mode, to its default value of no.
	///   I don’t know, man.
	/// - `CSI ?4l` (Reset to Jump Scrolling), which sets the scrolling mode to jump. This is naught
	///   to do with what we think of as “scrolling,” but rather it’s about the speed at which the
	///   terminal will add lines to the screen. Jump mode means “give it to me as fast as it comes”
	///   and Smooth mode means to do some buffering and output lines “at a moderate, smooth rate.”
	/// - `CSI 4l` (Reset to Replace Mode), which sets the cursor writing mode to Replace, i.e.
	///   overwriting characters at cursor position, instead of Insert, which pushes characters
	///   under the cursor to the right.
	/// - `ESC >` (Set Key Pad to Normal), which sets the keyboard’s numeric keypad to send “what’s
	///   printed on the keys” i.e. numbers and the arithmetic symbols.
	/// - `CSI ?69l` (Reset Left and Right Margins to the page), which sets the horizontal margins
	///   to coincide with the page’s margins: nowadays, no margins.
	XtermReset,

	/// Calls the command `tput clear`.
	///
	/// That command most likely does what [`Terminfo`][ClearScreen::Terminfo] does internally, but
	/// may work better in some cases, such as when the terminfo database on the system is hashed or
	/// in a non-standard location that the terminfo crate does not find.
	///
	/// However, it relies on the `tput` command being available, and on being able to run commands.
	TputClear,

	/// Calls the command `tput reset`.
	///
	/// See the documentation above on [`TputClear`][ClearScreen::TputClear] for more details, save
	/// that the equivalent is [`TerminfoReset`][ClearScreen::TerminfoReset].
	TputReset,

	/// Calls the command `cls`.
	///
	/// This is the Windows command to clear the screen. It has the same caveats as
	/// [`TputClear`][ClearScreen::TputClear] does, but its internal mechanism is not known. Prefer
	/// [`WindowsClear`][ClearScreen::WindowsClear] instead to avoid relying on an external command.
	///
	/// This will always attempt to run the command, regardless of compile target, which may have
	/// unintended effects if the `cls` executable does something different on the platform.
	Cls,

	/// Sets the Windows Console to support VT escapes.
	///
	/// This sets the `ENABLE_VIRTUAL_TERMINAL_PROCESSING` bit in the console mode, which enables
	/// support for the terminal escape sequences every other terminal uses. This is supported since
	/// Windows 10, from the Threshold 2 Update in November 2015.
	///
	/// Does nothing on non-Windows targets.
	WindowsVt,

	/// Sets the Windows Console to support VT escapes and prints the clear sequence.
	///
	/// This runs [`WindowsVt`][ClearScreen::WindowsVt] and [`XtermClear`][ClearScreen::XtermClear],
	/// in this order. This is described here:
	/// https://docs.microsoft.com/en-us/windows/console/clearing-the-screen#example-1 as the
	/// recommended clearing method for all new development, although we also reset the cursor
	/// position.
	///
	/// While `WindowsVt` will do nothing on non-Windows targets, `XtermClear` will still run.
	WindowsVtClear,

	/// Uses Windows Console function to scroll the screen buffer and fill it with white space.
	///
	/// - Scrolls up one screenful
	/// - Fills the buffer with whitespace and attributes set to default.
	/// - Flushes the input buffer
	/// - Sets the cursor position to 0,0
	///
	/// This is described here: https://docs.microsoft.com/en-us/windows/console/clearing-the-screen#example-2
	/// as the equivalent to CMD.EXE's `cls` command.
	///
	/// Does nothing on non-Windows targets.
	WindowsConsoleClear,

	/// Uses Windows Console function to blank the screen state.
	///
	/// - Fills the screen buffer with ` ` (space) characters
	/// - Resets cell attributes over the entire buffer
	/// - Flushes the input buffer
	/// - Sets the cursor position to 0,0
	///
	/// This is described here: https://docs.microsoft.com/en-us/windows/console/clearing-the-screen#example-3
	///
	/// Does nothing on non-Windows targets.
	WindowsConsoleBlank,

	/// Uses Windows Console function to disable raw mode.
	///
	/// Does nothing on non-Windows targets.
	WindowsCooked,

	/// Prints the RIS VT100 escape code: Reset to Initial State.
	///
	/// This is the `ESC c` or `1b 63` escape, which by spec is defined to reset the terminal state
	/// to all initial values, which may be a range of things, for example as described in the VT510
	/// manual: https://vt100.net/docs/vt510-rm/RIS
	///
	/// However, the exact behaviour is highly dependent on the terminal emulator, and some modern
	/// terminal emulators do not always clear scrollback, for example Tmux and GNOME VTE.
	VtRis,

	/// Prints the CSI sequence to leave the Alternate Screen mode.
	///
	/// If the screen is in alternate screen mode, like how vim or a pager or another such rich TUI
	/// application would do, this sequence will clear the alternate screen buffer, then revert the
	/// terminal to normal mode, and restore the position of the cursor to what it was before
	/// Alternate Screen mode was entered, assuming the proper sequence was used.
	///
	/// It will not clear the normal mode buffer.
	///
	/// This is useful when recovering from a TUI application which crashed without resetting state.
	VtLeaveAlt,

	/// Sets the terminal to cooked mode.
	///
	/// This attempts to switch the terminal to “cooked” mode, which can be thought of as the
	/// opposite of “raw” mode, where the terminal does not respond to line discipline (which makes
	/// carriage return, line feed, and general typing display out to screen, and translates Ctrl-C
	/// to sending the SIGINT signal, etc) but instead passes all input to the controlling program
	/// and only displays what it outputs explicitly.
	///
	/// There’s also an intermediate “cbreak” or “rare” mode which behaves like “cooked” but sends
	/// each character one at a time immediately rather buffering and sending lines.
	///
	/// TUI applications such as editors and pagers often set raw mode to gain precise control of
	/// the terminal state. If such a program crashes, it may not reset the terminal mode back to
	/// the mode it found it in, which can leave the terminal behaving oddly or rendering it
	/// completely unusable.
	///
	/// In truth, these terminal modes are a set of configuration bits that are given to the
	/// `termios(3)` libc API, and control a variety of terminal modes. “Cooked” mode sets:
	///
	/// - Input BRKINT set: on BREAK, flush i/o queues and send a SIGINT to any running process.
	/// - Input ICRNL set: translate Carriage Returns to New Lines on input.
	/// - Input IGNPAR set: ignore framing and parity errors.
	/// - Input ISTRIP set: strip off eigth bit.
	/// - Input IXON set: enable XON/XOFF flow control on output.
	/// - Output OPOST set: enable output processing.
	/// - Local ICANON set: enable canonical mode (see below).
	/// - Local ISIG set: when Ctrl-C, Ctrl-Q, etc are received, send the appropriate signal.
	///
	/// Canonical mode is really the core of “cooked” mode and enables:
	///
	/// - line buffering, so input is only sent to the underlying program when a line delimiter
	///   character is entered (usually a newline);
	/// - line editing, so ERASE (backspace) and KILL (remove entire line) control characters edit
	///   the line before it is sent to the program;
	/// - a maximum line length of 4096 characters (bytes).
	///
	/// When canonical mode is unset (when the bit is cleared), all input processing is disabled.
	///
	/// Due to how the underlying [`tcsetattr`] function is defined in POSIX, this may complete
	/// without error if _any part_ of the configuration is applied, not just when all of it is set.
	///
	/// Note that you generally want [`VtWellDone`][ClearScreen::VtWellDone] instead.
	///
	/// Does nothing on non-Unix targets.
	///
	/// [`tcsetattr`]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/tcsetattr.html
	VtCooked,

	/// Sets the terminal to “well done” mode.
	///
	/// This is similar to [`VtCooked`][ClearScreen::VtCooked], but with a different, broader, mode
	/// configuration which approximates a terminal’s initial state, such as is expected by a shell,
	/// and clears many bits that should probably never be set (like the translation/mapping modes).
	///
	/// “Well done” mode is an invention of this library, inspired by several other sources such as
	/// Golang’s goterm, the termios(3) and tput(1) manual pages, but not identical to any.
	///
	/// Notably most implementations read the terminal configuration bits and only modify that set,
	/// whereas this library authoritatively writes the entire configuration from scratch.
	///
	/// It is a strict superset of [`VtCooked`][ClearScreen::VtCooked].
	///
	/// - Input BRKINT set: on BREAK, flush i/o queues and send a SIGINT to any running process.
	/// - Input ICRNL set: translate Carriage Return to New Line on input.
	/// - Input IUTF8 set: input is UTF-8 (Linux only, since 2.6.4).
	/// - Input IGNPAR set: ignore framing and parity errors.
	/// - Input IMAXBEL set: ring terminal bell when input queue is full (not implemented in Linux).
	/// - Input ISTRIP set: strip off eigth bit.
	/// - Input IXON set: enable XON/XOFF flow control on output.
	/// - Output ONLCR set: do not translate Carriage Return to CR NL.
	/// - Output OPOST set: enable output processing.
	/// - Control CREAD set: enable receiver.
	/// - Local ICANON set: enable canonical mode (see [`VtCooked`][ClearScreen::VtCooked]).
	/// - Local ISIG set: when Ctrl-C, Ctrl-Q, etc are received, send the appropriate signal.
	///
	/// Does nothing on non-Unix targets.
	VtWellDone,
}

impl Default for ClearScreen {
	fn default() -> Self {
		todo!()
	}
}

const ESC: &[u8] = b"\x1b";
const CSI: &[u8] = b"\x1b[";
const RIS: &[u8] = b"c";

impl ClearScreen {
	/// Performs the clearing action, printing to stdout.
	pub fn clear(self) -> Result<(), Error> {
		let mut stdout = io::stdout();
		self.clear_to(&mut stdout)
	}

	/// Performs the clearing action, printing to a given writer.
	///
	/// This allows to capture any escape sequences that might be printed, for example, but note
	/// that it will not prevent actions taken via system APIs, such as the Windows, VtCooked, and
	/// VtWellDone variants do.
	///
	/// For normal use, prefer [`clear()`].
	pub fn clear_to(self, mut w: &mut impl Write) -> Result<(), Error> {
		match self {
			Self::Terminfo => {
				let info = Database::from_env()?;
				let mut ctx = Context::default();

				if let Some(seq) = info.get::<capability::ClearScreen>() {
					seq.expand().with(&mut ctx).to(&mut w)?;
				} else {
					return Err(Error::TerminfoCap("clear"));
				}

				if let Some(seq) = info.get::<capability::User3>() {
					seq.expand().with(&mut ctx).to(w)?;
				}
			}
			Self::TerminfoScreen => {
				let info = Database::from_env()?;
				if let Some(seq) = info.get::<capability::ClearScreen>() {
					seq.expand().to(w)?;
				} else {
					return Err(Error::TerminfoCap("clear"));
				}
			}
			Self::TerminfoScrollback => {
				let info = Database::from_env()?;
				if let Some(seq) = info.get::<capability::User3>() {
					seq.expand().to(w)?;
				} else {
					return Err(Error::TerminfoCap("E3"));
				}
			}
			Self::TerminfoReset => {
				let info = Database::from_env()?;
				let mut ctx = Context::default();
				let mut reset = false;

				if let Some(seq) = info.get::<capability::Reset1String>() {
					reset = true;
					seq.expand().with(&mut ctx).to(&mut w)?;
				}
				if let Some(seq) = info.get::<capability::Reset2String>() {
					reset = true;
					seq.expand().with(&mut ctx).to(&mut w)?;
				}
				if let Some(seq) = info.get::<capability::Reset3String>() {
					reset = true;
					seq.expand().with(&mut ctx).to(&mut w)?;
				}
				if let Some(seq) = info.get::<capability::ResetFile>() {
					reset = true;
					seq.expand().with(&mut ctx).to(&mut w)?;
				}

				if reset {
					return Ok(());
				}

				if let Some(seq) = info.get::<capability::Init1String>() {
					reset = true;
					seq.expand().with(&mut ctx).to(&mut w)?;
				}
				if let Some(seq) = info.get::<capability::Init2String>() {
					reset = true;
					seq.expand().with(&mut ctx).to(&mut w)?;
				}
				if let Some(seq) = info.get::<capability::Init3String>() {
					reset = true;
					seq.expand().with(&mut ctx).to(&mut w)?;
				}
				if let Some(seq) = info.get::<capability::InitFile>() {
					reset = true;
					seq.expand().with(&mut ctx).to(&mut w)?;
				}

				if !reset {
					return Err(Error::TerminfoCap("reset"));
				}
			}
			Self::XtermClear => {
				const CURSOR_HOME: &[u8] = b"H";
				const ERASE_SCREEN: &[u8] = b"2J";
				const ERASE_SCROLLBACK: &[u8] = b"3J";

				w.write_all(CSI)?;
				w.write_all(CURSOR_HOME)?;

				w.write_all(CSI)?;
				w.write_all(ERASE_SCREEN)?;

				w.write_all(CSI)?;
				w.write_all(ERASE_SCROLLBACK)?;
			}
			Self::XtermReset => {
				const STR: &[u8] = b"!p";
				const RESET_WIDTH_AND_SCROLL: &[u8] = b"?3;4l";
				const RESET_REPLACE: &[u8] = b"4l";
				const RESET_KEYPAD: &[u8] = b">";
				const RESET_MARGINS: &[u8] = b"?69l";

				w.write_all(ESC)?;
				w.write_all(RIS)?;

				w.write_all(CSI)?;
				w.write_all(STR)?;

				w.write_all(CSI)?;
				w.write_all(RESET_WIDTH_AND_SCROLL)?;

				w.write_all(CSI)?;
				w.write_all(RESET_REPLACE)?;

				w.write_all(ESC)?;
				w.write_all(RESET_KEYPAD)?;

				w.write_all(CSI)?;
				w.write_all(RESET_MARGINS)?;
			}
			Self::TputClear => {
				let status = Command::new("tput").arg("clear").status()?;
				if !status.success() {
					return Err(Error::Command("tput clear", status));
				}
			}
			Self::TputReset => {
				let status = Command::new("tput").arg("reset").status()?;
				if !status.success() {
					return Err(Error::Command("tput reset", status));
				}
			}
			Self::Cls => {
				let status = Command::new("cls").status()?;
				if !status.success() {
					return Err(Error::Command("cls", status));
				}
			}
			Self::WindowsVt => win::vt()?,
			Self::WindowsVtClear => {
				let vtres = win::vt();
				Self::XtermClear.clear_to(w)?;
				vtres?;
			}
			Self::WindowsConsoleClear => win::clear()?,
			Self::WindowsConsoleBlank => win::blank()?,
			Self::WindowsCooked => win::cooked()?,
			Self::VtRis => {
				w.write_all(ESC)?;
				w.write_all(RIS)?;
			}
			Self::VtLeaveAlt => {
				const LEAVE_ALT: &[u8] = b"?1049l";
				w.write_all(CSI)?;
				w.write_all(LEAVE_ALT)?;
			}
			Self::VtCooked => unix::vt_cooked()?,
			Self::VtWellDone => unix::vt_well_done()?,
		}

		Ok(())
	}
}

/// Shorthand for `ClearScreen::default().clear()`.
pub fn clear() -> Result<(), Error> {
	ClearScreen::default().clear()
}

/// Error type.
#[derive(Debug, Error)]
pub enum Error {
	/// Any I/O error.
	#[error(transparent)]
	Io(#[from] io::Error),

	/// A non-success exit status from a command.
	#[error("{0}: {1}")]
	Command(&'static str, ExitStatus),

	/// Any nix (libc) error.
	#[cfg(unix)]
	#[error(transparent)]
	Nix(#[from] nix::Error),

	/// Any terminfo error.
	#[error(transparent)]
	Terminfo(#[from] terminfo::Error),

	/// A missing terminfo capability.
	#[error("required terminfo capability not available: {0}")]
	TerminfoCap(&'static str),
}

#[cfg(unix)]
mod unix {
	use super::Error;

	use nix::{
		libc::STDIN_FILENO,
		sys::termios::{
			tcgetattr, tcsetattr, ControlFlags, InputFlags, LocalFlags, OutputFlags,
			SetArg::TCSANOW, Termios,
		},
		unistd::isatty,
	};

	use std::{fs::OpenOptions, os::unix::prelude::AsRawFd};

	pub(crate) fn vt_cooked() -> Result<(), Error> {
		write_termios(|t| {
			t.input_flags.insert(
				InputFlags::BRKINT
					| InputFlags::ICRNL | InputFlags::IGNPAR
					| InputFlags::ISTRIP | InputFlags::IXON,
			);
			t.output_flags.insert(OutputFlags::OPOST);
			t.local_flags.insert(LocalFlags::ICANON | LocalFlags::ISIG);
		})
	}

	pub(crate) fn vt_well_done() -> Result<(), Error> {
		write_termios(|t| {
			t.input_flags.insert(
				InputFlags::BRKINT
					| InputFlags::ICRNL | InputFlags::IUTF8
					| InputFlags::IGNPAR | InputFlags::IMAXBEL
					| InputFlags::ISTRIP | InputFlags::IXON,
			);
			t.output_flags
				.insert(OutputFlags::ONLCR | OutputFlags::OPOST);
			t.control_flags.insert(ControlFlags::CREAD);
			t.local_flags.insert(LocalFlags::ICANON | LocalFlags::ISIG);
		})
	}

	fn reset_termios(t: &mut Termios) {
		t.input_flags.remove(InputFlags::all());
		t.output_flags.remove(OutputFlags::all());
		t.control_flags.remove(ControlFlags::all());
		t.local_flags.remove(LocalFlags::all());
	}

	fn write_termios(f: impl Fn(&mut Termios)) -> Result<(), Error> {
		if isatty(STDIN_FILENO)? {
			let mut t = tcgetattr(STDIN_FILENO)?;
			reset_termios(&mut t);
			f(&mut t);
			tcsetattr(STDIN_FILENO, TCSANOW, &t)?;
		} else {
			let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
			let fd = tty.as_raw_fd();

			let mut t = tcgetattr(fd)?;
			reset_termios(&mut t);
			f(&mut t);
			tcsetattr(fd, TCSANOW, &t)?;
		}

		Ok(())
	}
}

#[cfg(windows)]
mod win {
	use super::Error;

	pub(crate) fn vt() -> Result<(), Error> {
		todo!()
	}

	pub(crate) fn clear() -> Result<(), Error> {
		todo!()
	}

	pub(crate) fn blank() -> Result<(), Error> {
		todo!()
	}

	pub(crate) fn cooked() -> Result<(), Error> {
		todo!()
	}
}

#[cfg(not(unix))]
#[allow(clippy::clippy::unnecessary_wraps)]
mod unix {
	use super::Error;

	pub(crate) fn vt_cooked() -> Result<(), Error> {
		Ok(())
	}

	pub(crate) fn vt_well_done() -> Result<(), Error> {
		Ok(())
	}
}

#[cfg(not(windows))]
#[allow(clippy::clippy::unnecessary_wraps)]
mod win {
	use super::Error;

	pub(crate) fn vt() -> Result<(), Error> {
		Ok(())
	}

	pub(crate) fn clear() -> Result<(), Error> {
		Ok(())
	}

	pub(crate) fn blank() -> Result<(), Error> {
		Ok(())
	}

	pub(crate) fn cooked() -> Result<(), Error> {
		Ok(())
	}
}
