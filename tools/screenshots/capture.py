"""Drive gp-atlas in a pty (pyte) and dump colored screens as JSON."""
import os, pty, sys, time, select, fcntl, termios, struct, json, pyte
COLS, ROWS = int(os.environ.get("COLS", 132)), int(os.environ.get("ROWS", 24))
steps = json.loads(os.environ["STEPS"]); out = os.environ["OUT"]
pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"; os.environ["COLORTERM"] = "truecolor"
    os.execvp(sys.argv[1], sys.argv[1:])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
scr = pyte.Screen(COLS, ROWS); st = pyte.ByteStream(scr)
def pump(t):
    end = time.time() + t
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try: st.feed(os.read(fd, 65536))
            except OSError: return
def dump(name):
    rows = []
    for y in range(scr.lines):
        line = []
        for x in range(scr.columns):
            c = scr.buffer[y][x]
            line.append([c.data or " ", c.fg, c.bg, c.bold, c.reverse, c.italics])
        rows.append(line)
    json.dump(rows, open(os.path.join(out, name + ".json"), "w"))
pump(2.5)
for kind, val, wait in steps:
    if kind == "key": os.write(fd, val.encode())
    elif kind == "click":
        x, y = val; os.write(fd, f"\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m".encode())
    if kind == "dump": dump(val)
    pump(wait)
os.write(fd, b"\x1b\x1b\x1bq"); pump(0.5)
