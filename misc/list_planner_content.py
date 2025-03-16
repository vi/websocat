import sys

from mytypes import *

def document_planner_content(c: PlannerContent) -> None:
    print("Short list of endpoint prefixes:")

    for ep in c.endpoints:
        if ep.prefixes:
            print(f"  {ep.prefixes[0]}")

    
    print()
    print("Short list of overlay prefixes:")
    for ep in c.overlays:
        if ep.prefixes:
            print(f"  {ep.prefixes[0]}")
            if ep.prefixes[0] == "ws-lowlevel-client:":
                print("  ws-lowlevel-server:")


def main() -> None:
    c = sys.stdin.read()
    things  = ThingsToDocument.from_json(c)

    planner_content: PlannerContent = things.planner_content

    planner_content.endpoints.sort(key=lambda x:x.name)
    planner_content.overlays.sort(key=lambda x:x.name)

    document_planner_content(planner_content)

if __name__ == '__main__':
    main()
