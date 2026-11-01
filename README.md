# Vier

Vier is four in a row for the Linux desktop: drop red discs into a seven-by-six board against yellow, and line up four across, down or diagonally first. The opponent is the computer, or a second person at the same keyboard. Vier has no account, no network, no ads and no analytics. It picks up the game in progress where it was left, and has an Easy level built for beginners.

It is the desktop sibling of [Vier for iPhone and Apple Watch](https://github.com/excelano/vier), with the same rules, the same opponent and the same look. Nothing passes between the two.

## Install

On Debian and Ubuntu (amd64), from the Excelano apt repository:

```
curl -fsSL https://excelano.com/apt/setup.sh | sudo sh && sudo apt install vier
```

That installs the game and a launcher named Vier.

## Playing

Move the pointer across the board, or use the left and right arrow keys or 1 to 7, to put the red disc over a column. Click, or press Space, Enter or the down arrow, to drop it. The computer thinks for a moment, slides its yellow disc over and drops it. Who moves first alternates from game to game. When someone lines up four, those discs are ringed and pulse, and the game offers Play again or Home. Escape returns home at any time, and the game waits there.

Easy, Medium and Hard set how far ahead the computer looks and how often it plays a random move instead. Every level takes a winning move when it has one; Medium and Hard also always block yours. Choosing Two players on the home screen hands the board back and forth between red and yellow, with no difficulty setting.

## Building

`cargo build --release` builds the window, and `cargo test` runs the rules and opponent tests, which need no display. `cargo deb` builds the package. `packaging/linux/check-libraries.sh` runs the release build under Wayland and X11 and refuses if a library it opens is not reachable from the package's `depends`, since the display stack is opened by name at run time and the linker never sees it.

The game keeps its state in `$XDG_STATE_HOME/vier/state.json`, which is `~/.local/state/vier/state.json` by default: the chosen difficulty, whether one or two players are at the board, who moves first next, and the game in progress.

## License

MIT. See [LICENSE](LICENSE).
