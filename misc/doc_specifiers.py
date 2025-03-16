import os
import subprocess
import sys

from typing import Generator, Tuple, List, Dict, Set, TypeVar
from dataclasses import dataclass
from collections import defaultdict
import re
from enum import Enum, unique, auto
from pprint import pprint

from mytypes import *

def document_planner_content(c: PlannerContent) -> None:

    def document_item(ep: PlannerItem, typnam: str) -> None:
        print(f"## {ep.name}") 
        print()
        inhibit_prefixes = False
        if not ep.doc:
            print("(undocumented)")
            print()
        else:
            d = ep.doc
            if d.find("@inhibit_prefixes") > -1:
                inhibit_prefixes = True
                d = d.replace("@inhibit_prefixes", "")
            print(d)
            print()
        if not inhibit_prefixes:
            if not ep.prefixes:
                print(f"This {typnam} cannot be directly specified as a "+
                    "prefix to a positional CLI argument, "+
                    "there may be some other way to access it.")
                print()
            else:
                print("Prefixes:")
                print()
                for prefix in ep.prefixes:
                    print(f"* `{prefix}`")
                print()

    if os.getenv("TODOC") == "endpoints":
        for ep in c.endpoints:
            document_item(ep, "endpoint")
    elif os.getenv("TODOC") == "overlays":    
        for ep in c.overlays:
            document_item(ep, "overlay")
    else:
        print("TODOC environment variable should be `endpoints` or `overlays`")

def main() -> None:
    
    c = sys.stdin.read()
    things  = ThingsToDocument.from_json(c)
    #pprint(things)

    planner_content: PlannerContent = things.planner_content

    planner_content.endpoints.sort(key=lambda x:x.name)
    planner_content.overlays.sort(key=lambda x:x.name)

    document_planner_content(planner_content)



if __name__ == '__main__':
    main()
