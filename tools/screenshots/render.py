"""Turn a captured screen into a terminal-window HTML page."""
import json, sys, html, unicodedata
PAL = {"black": "#1b1f27", "red": "#ef6b73", "green": "#8fd18b", "brown": "#e9c46a", "yellow": "#e9c46a",
       "blue": "#6cb6ff", "magenta": "#c792ea", "cyan": "#5ccfe6", "white": "#d6dbe4",
       "brightblack": "#6b7383", "brightred": "#ff8087", "brightgreen": "#a6e3a1", "brightbrown": "#f6d47c",
       "brightyellow": "#f6d47c", "brightblue": "#8ccaff", "brightmagenta": "#dcb4ff", "brightcyan": "#89ddff",
       "brightwhite": "#ffffff"}
FG, BG = "#c9d1d9", "#0f141b"
def col(c, default):
    if c == "default": return default
    if c in PAL: return PAL[c]
    if len(c) == 6: return "#" + c
    return default
rows = json.load(open(sys.argv[1])); title = sys.argv[3] if len(sys.argv) > 3 else "gp-atlas"
out = []
for row in rows:
    spans, cur, buf = [], None, ""
    skip = False
    for i, (ch, fg, bg, bold, rev, ital) in enumerate(row):
        if skip or ch == "":
            skip = False
            continue  # second half of a wide character
        # The terminal gives emoji such as ✅ two cells; pyte may store one
        # cell plus a blank, so use the Unicode width to keep columns aligned.
        nxt = row[i + 1][0] if i + 1 < len(row) else None
        wide = nxt == "" or (len(ch) == 1 and unicodedata.east_asian_width(ch) in "WF" and nxt == " ")
        skip = wide and nxt == " "
        f, b = col(fg, FG), col(bg, BG)
        if rev: f, b = (b if bg != "default" else BG), (col(fg, FG))
        key = (f, b, bold)
        if (key != cur or wide) and buf:
            spans.append((cur, buf, False)); buf = ""
        cur = key
        if wide:
            spans.append((key, ch, True))
        else:
            buf += ch
    if buf: spans.append((cur, buf, False))
    out.append("".join(
        f'<span style="color:{k[0]};background:{k[1]};{"font-weight:700;" if k[2] else ""}'
        + ('display:inline-block;width:2ch;text-align:center;overflow:hidden;vertical-align:top' if w else '')
        + f'">{html.escape(t)}</span>' for k, t, w in spans))
page = f"""<!doctype html><meta charset="utf-8"><style>
body{{margin:0;background:transparent;padding:24px;display:inline-block}}
.win{{background:{BG};border-radius:12px;box-shadow:0 18px 50px rgba(0,0,0,.45),0 0 0 1px rgba(255,255,255,.06);overflow:hidden;display:inline-block}}
.bar{{height:34px;background:#1a212b;display:flex;align-items:center;padding:0 14px;gap:8px;position:relative}}
.d{{width:12px;height:12px;border-radius:50%}}
.t{{position:absolute;left:0;right:0;text-align:center;color:#8b95a5;font:13px -apple-system,Segoe UI,Helvetica,Arial,sans-serif;pointer-events:none}}
pre{{margin:0;padding:10px 12px 12px;font:13.5px/1.32 'DejaVu Sans Mono','Menlo',monospace;color:{FG}}}
</style><div class="win"><div class="bar"><span class="d" style="background:#ff5f57"></span><span class="d" style="background:#febc2e"></span><span class="d" style="background:#28c840"></span><div class="t">{html.escape(title)}</div></div><pre>{chr(10).join(out)}</pre></div>"""
open(sys.argv[2], "w").write(page)
