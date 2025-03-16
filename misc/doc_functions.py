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


E_FN_COOK_RET1 = re.compile(r"""
    \s* -> \s* (.*)
    """, re.VERBOSE)
E_FN_COOK_RET2 = re.compile(r"""
    ^ RhResult \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)
E_STRIP_HANDLE = re.compile(r"""
    ^ Handle \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)


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
        for x in f.params:
            nam = x.nam
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
        for od in f.options:
            on = od.nam
            on = on.removeprefix("r#")
            s = "* " + on + " (`" + od.typ + "`)"
            if od.doc:
                s += ' - '
                s += od.doc
            print(s)
        print()


def main() -> None:
    c = sys.stdin.read()
    things  = ThingsToDocument.from_json(c)
    #pprint(things)

    executor_functions : List[ExecutorFunc] = things.executor_functions
    executor_functions.sort(key=lambda x: x.rhai_function)

    for execfn in executor_functions:
        document_executor_function(execfn)

if __name__ == '__main__':
    main()
