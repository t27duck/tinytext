#!/usr/bin/env python3
"""Drive a running tinytext through the accessibility bus (for testing).

Usage:
    scripts/atspi.py dump              # print tinytext's widget tree
    scripts/atspi.py type <text>       # insert text at the start of the first text view
    scripts/atspi.py click <label>     # activate the last button with this label
"""
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi


def find_app():
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app and app.get_name() == "tinytext":
            return app
    sys.exit("tinytext is not on the accessibility bus")


def walk(node, depth=0, out=None):
    out = [] if out is None else out
    out.append((depth, node))
    for i in range(node.get_child_count()):
        child = node.get_child_at_index(i)
        if child:
            walk(child, depth + 1, out)
    return out


def main():
    cmd, args = sys.argv[1], sys.argv[2:]
    nodes = walk(find_app())
    if cmd == "dump":
        for depth, n in nodes:
            print("  " * depth + n.get_role_name(), repr(n.get_name()))
    elif cmd == "type":
        text = next(n for _, n in nodes if n.get_role_name() == "text")
        text.get_editable_text_iface().insert_text(0, args[0], len(args[0]))
    elif cmd == "click":
        # Last match, so a dialog's "Close" wins over the window's titlebar button.
        button = next(
            n for _, n in reversed(nodes)
            if n.get_role_name() == "button" and n.get_name() == args[0]
        )
        button.get_action_iface().do_action(0)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
