"""A terminal program that asks to be told of color-scheme changes (mode 2031) and, on each report, queries the
background color (OSC 11), printing both: what a program in Sapphire's terminal sees when the appearance switches."""
import os
import re
import sys
import termios
import tty

fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
tty.setraw(fd)
REPORT = re.compile(rb"\x1b\[\?997;([12])n")
BACKGROUND = re.compile(rb"\x1b\]11;(rgb:[0-9a-fA-F/]+)(?:\x07|\x1b\\)")
try:
    os.write(1, b"\x1b[?2031h")
    os.write(1, b"LISTENING\r\n")
    pending = b""
    scheme = None
    while True:
        pending += os.read(fd, 256)
        while True:
            report, background = REPORT.search(pending), BACKGROUND.search(pending)
            first = min((m for m in (report, background) if m), key=lambda m: m.start(), default=None)
            if first is None:
                break
            pending = pending[first.end():]
            if first is report:
                scheme = b"dark" if report.group(1) == b"1" else b"light"
                os.write(1, b"\x1b]11;?\x1b\\")
            else:
                os.write(1, b"SCHEME " + (scheme or b"?") + b" BG " + background.group(1) + b"\r\n")
finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
