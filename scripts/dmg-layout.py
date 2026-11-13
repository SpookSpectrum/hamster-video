#!/usr/bin/env python3
"""Write the minimal left-to-right Finder installer layout (build dependency only)."""
import sys
from ds_store import DSStore

with DSStore.open(sys.argv[1], "w+") as store:
    store["."]["bwsp"] = {
        "ShowStatusBar": False, "ShowToolbar": False, "ShowTabView": False,
        "ShowPathbar": False, "ShowSidebar": False, "ContainerShowSidebar": False,
        "WindowBounds": "{{200, 180}, {640, 380}}", "SidebarWidth": 0,
    }
    store["."]["icvp"] = {
        "viewOptionsVersion": 1, "backgroundType": 1,
        "backgroundColorRed": 1.0, "backgroundColorGreen": 1.0,
        "backgroundColorBlue": 1.0, "iconSize": 96.0, "textSize": 14.0,
        "gridOffsetX": 0.0, "gridOffsetY": 0.0, "gridSpacing": 100.0,
        "arrangeBy": "none", "labelOnBottom": True,
        "showIconPreview": True, "showItemInfo": False,
    }
    store["."]["vSrn"] = ("long", 1)
    store["."]["vstl"] = ("type", b"icnv")
    store["Hamster Video.app"]["Iloc"] = (160, 170)
    store["Applications"]["Iloc"] = (480, 170)
