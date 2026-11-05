"""Unix acceptance rules for known application raw-mode restoration paths."""
import sys
import termios


def raw_terminal_restored(saved, actual):
    """Accept exact state, or Darwin's solely newly added canonical PENDIN.

    Only use after a known raw-to-saved-mode restoration. Do not normalize the
    observations: losing an originally saved PENDIN or any other delta fails.
    """
    if actual == saved:
        return True
    if sys.platform != "darwin" or not saved[3] & termios.ICANON or saved[3] & termios.PENDIN:
        return False
    expected = list(saved)
    expected[3] |= termios.PENDIN
    return actual == expected
