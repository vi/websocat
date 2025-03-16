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

E_METHOD_SELF = re.compile(r"""
    ^ \& \s* mut \s* Handle \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)


E_FN_COOK_RET1 = re.compile(r"""
    \s* -> \s* (.*)
    """, re.VERBOSE)
E_FN_COOK_RET2 = re.compile(r"""
    ^ RhResult \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)
E_STRIP_HANDLE = re.compile(r"""
    ^ Handle \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)


############################################################################################

############################################################################################

############################################################################################

@dataclass
class TypeAndDoc:
    typ: str
    doc: str


@dataclass
class ExecutorFuncCallback:
    params: List[str]
    rettyp: str

@dataclass
class ExecutorFunc:
    rust_function: str
    rhai_function: str
    primary_doc: str
    params: List[Tuple[str, TypeAndDoc]]
    ret: TypeAndDoc
    callbacks: Dict[str, ExecutorFuncCallback]
    options: List[Tuple[str, TypeAndDoc]]

def strip_handle(s : str) -> str:
    if x:=E_STRIP_HANDLE.search(s):
        s=x.group(1)
    return s.strip()

def document_executor_function(f: ExecutorFunc) -> None:
    if "doc(hidden)" in f.primary_doc:
        return
    print("## " + f.rhai_function)
    print("")
    if f.primary_doc != "":
        print(f.primary_doc)
        print()
    if len(f.params) > 0:
        print("Parameters:")
        print()
        for (nam, x) in f.params:
            if nam == "opts" and x.typ == "Dynamic" and not x.doc:
                x.doc = "object map containing dynamic options to the function"
            nam = nam.removeprefix("r#")
            s = "* " + nam + " (`" 
            if x.typ != "FnPtr":
                s += strip_handle(x.typ)
            else:
                if nam in f.callbacks:
                    cbinfo = f.callbacks[nam]
                    s += "Fn("
                    for (i,pt) in enumerate(cbinfo.params):
                        if i>0:
                            s += ", "
                        s += strip_handle(pt.strip())
                    s += ")"
                    if cbinfo.rettyp:
                        s += " -> "
                        s += strip_handle(cbinfo.rettyp)
                else:
                    s+="???"
                if not x.doc:
                    x.doc = "Rhai function that will be called to continue processing"
            s += "`)"
            if x.doc:
                s += " - " + x.doc
            print(s)
        print()

    s=""
    if not f.ret.typ or f.ret.typ == '()':
        s="Does not return anything."
    else:
        r = f.ret.typ
        if xx := E_FN_COOK_RET1.search(r):
            r = xx.group(1)
        if xx := E_FN_COOK_RET2.search(r):
            r = xx.group(1)
        s="Returns `" + strip_handle(r) + '`'
    if f.ret.doc:
        s += " - " + f.ret.doc
    print(s)
    print()
    if len(f.options) > 0:
        print("Options:")
        print()
        for (on, od) in f.options:
            on = on.removeprefix("r#")
            s = "* " + on + " (`" + od.typ + "`)"
            if od.doc:
                s += ' - '
                s += od.doc
            print(s)
        print()


############################################################################################

############################################################################################

############################################################################################

@dataclass
class PlannerItem:
    name: str
    prefixes: List[str]
    doc: str

@dataclass
class PlannerContent:
    endpoints: List[PlannerItem]
    overlays: List[PlannerItem]

def document_planner_content(c: PlannerContent) -> None:

    def document_item(ep: PlannerItem, typnam: str) -> None:
        print(f"### {ep.name}") 
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

    f = open("src/help_addendum.txt","w")
    f.write("Short list of endpoint prefixes:\n")

    print()
    print("## Endpoints")
    print()

    for ep in c.endpoints:
        if ep.prefixes:
            f.write(f"  {ep.prefixes[0]}\n")
        document_item(ep, "endpoint")

    print()
    print("## Overlays")
    print()
    
    f.write("\n")
    f.write("Short list of overlay prefixes:\n")
    for ep in c.overlays:
        if ep.prefixes:
            f.write(f"  {ep.prefixes[0]}\n")
            if ep.prefixes[0] == "ws-lowlevel-client:":
                f.write("  ws-lowlevel-server:\n")
        document_item(ep, "overlay")

    f.write("\n")
    f.write("Examples:\n")
    f.write("\n")
    f.write("  websocat ws://127.0.0.1:1234\n")
    f.write("    Simple WebSocket client\n")
    f.write("\n")
    f.write("  websocat -s 1234\n")
    f.write("    Simple WebSocket server\n")
    f.write("\n")
    f.write("  websocat -b tcp-l:127.0.0.1:1234 wss://ws.vi-server.org/mirror\n")
    f.write("    TCP-to-WebSocket converter\n")
    f.write("\n")
    f.write("  websocat -b ws-l:127.0.0.1:8080 udp:127.0.0.1:1234\n")
    f.write("    WebSocket-to-UDP converter\n")
    f.write("\n")
    f.write("Use doc.md for reference of all Websocat functions\n")

    f.close()

############################################################################################

############################################################################################

############################################################################################

def process_outline(o: Outline) -> Tuple[PlannerContent, List[ExecutorFunc]]:
    endpoints : List[PlannerItem] = []
    overlays : List[PlannerItem] = []
    funcs : List[ExecutorFunc] = []

    approved_functitons : Dict[str, str] = {}
    for f in o.functions:
        if f.name == 'register':
            for rc in f.reg_calls:
                approved_functitons[rc.fnname] = rc.rhname
    for f in o.functions:
        if f.name in approved_functitons:
            cbmap : Dict[ExecutorFuncCallback] = {}
            for (k, v) in f.callbacks.items():
                cbmap[k] = ExecutorFuncCallback(v[0].argtyps, v[0].rettyp)
            params = [ (x.name, TypeAndDoc(x.typ, " ".join(x.doc))) for x in f.args  ]
            params = [x for x in params if x[0] != "ctx"]
            displayname = approved_functitons[f.name]

            if params:
                firstparam = params[0]
                if mn := E_METHOD_SELF.search(firstparam[1].typ):
                    x = mn.group(1)
                    params.pop(0)
                    displayname=x + "::" + displayname

            funcs.append(ExecutorFunc(
                f.name,
                displayname,
                "\n".join(f.doc),
                params,
                TypeAndDoc(f.rettyp, " ".join(f.retdoc)),
                cbmap,
                [(x.name, TypeAndDoc(x.typ, " ".join(x.doc))) for x in f.opts]
            ))


    endpoint_prefixes : Dict[str, List[str]] = defaultdict(list)
    overlay_prefixes : Dict[str, List[str]] = defaultdict(list)
    for t in o.endpoint_prefixes:
        endpoint_prefixes[t.name].extend(t.prefixes)
    for t in o.overlay_prefixes:
        overlay_prefixes[t.name].extend(t.prefixes)

    for x in o.endpoints:
        endpoints.append(PlannerItem(x.ident, endpoint_prefixes.get(x.ident) or [], "\n".join(x.doc)))
    for x in o.overlays:
        overlays.append(PlannerItem(x.ident, overlay_prefixes.get(x.ident) or [], "\n".join(x.doc)))


    return (PlannerContent(endpoints, overlays), funcs)


def main() -> None:
    executor_functions : List[ExecutorFunc]
    planner_content: PlannerContent

    outline : Outline = Outline.from_json(sys.stdin.read())
    
    (planner_content,executor_functions) =  process_outline(outline)

    #pprint(outline)

    executor_functions.sort(key=lambda x: x.rhai_function)
    planner_content.endpoints.sort(key=lambda x:x.name)
    planner_content.overlays.sort(key=lambda x:x.name)

    print("# Command-line interface")
    print()
    print("This section describes options, flags and specifiers of Websocat CLI.")
    print()
    print("## `--help` output")
    print()
    print("```")
    sys.stdout.flush();
    subprocess.run(["./target/mydev/websocat","--help"])
    print("```")
    print()

    document_planner_content(planner_content)

    print("# Scenario functions")
    print()
    print("Prior to doing any network things, Websocat prepares a Scenario (Websocat Rhai Script) based on you command line options.")
    print("Scenarios are less stable than usual Websocat API, but allow fine tuning Websocat behaviour.")
    print("You can view scenarios using `--dump-spec` option and execute them the with `-x` option.")
    print()
    print("The following functions and methods are used in scenarios:")
    print()

    for execfn in executor_functions:
        document_executor_function(execfn)

    
    print(r"""
# Glossary

* Specifier - WebSocket URL, TCP socket address or other connection type Websocat recognizes, 
or an overlay that transforms other Specifier.
* Endpoint - leaf-level specifier that directly creates some sort of Socket, without requiring another Socket first.
* Overlay - intermediate specifier that transforms inner specifier. From overlay's viewpoint, inner socket is called Downstream and whatever uses the overlay is called Upstream.
* Socket - a pair of byte stream- or datagram-oriented data flows: write (sink) and read (source), optionally with a hangup signal. Can be stream- and packet-oriented.
* Incomplete socket - socket where one of direction (reader or writer) is absent (null). Not to be confused with half-shutdown socket that can be read, but not written.
* Scenario = Websocat Rhai Script - detailed instruction of how Websocat would perform its operation.
Normally it is generated automatically from CLI arguments, then executed; but you can separate 
those steps and customize the scenario to fine tune of how Websocat operates. Just like usual CLI API, 
Rhai functions API is also intended to be semver-stable API of Websocat.
* Scenario function - Rhai native function that Websocat registers with Rhai engine that can be used 
in Scenarios.
* Scenario Planner - part of Websocat implementation that parses command line arguments and prepares a Scenario
* Scenario Executor - part of Websocat implementation that executes a Scenario.
* CLI arguments - combination of a positional arguments (typically Specifiers) and various flags (e.g. `--binary`) and options (e.g. `--buffer-size 4096`) that affect Scenario Planner. Sometimes, in narrow sense, it may refer to an individual block of `--compose`-ed arguments.
* Packet = Datagram = Message - A byte buffer with associated flags. Correspond to one WebSocket message. Within WebSocket, packets can be split to chunks, but that should not affect user-visible properties.
* Chunk = Frame - portion of data read or written to/from stream or datagram socket in one go. Maybe a fragment of a Message or be the whole Message.
* Task - a logical thread of execution. Rhai code is expected to create and combine some tasks. Typically each connection runs in its own task. Corresponds to Tokio tasks.
* Hangup - similar to Task, but used in context of signaling various events, especially abrupt reset of sockets.
* Specifier stack - Invididual components of a Specifier - Endpoint and a vector of Overlays.
* Left side, first specifier - first positional argument you have specified at the left side of the Websocat CLI invocation (maybe after some transformation). Designed to handle both one-time use connectors and multi-use listeners.
* Right side, second specifier - second positional argument of the Websocat CLI invocation (may be auto-generated). Designed for single-use things to attach to connections obtained from the Left side.
* Listener - Type of Specifier that waits for incoming connections, swapning a task with a Socket for each incoming connection.

""")


if __name__ == '__main__':
    main()
