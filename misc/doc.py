import re
import os

from typing import List, Dict, Tuple
from dataclasses import dataclass
from enum import Enum

E_REGISTER_FN_START = re.compile(r"^pub fn register\s*\(")
E_REGISTER_FN_ENTRY = re.compile(r"""
    register_fn \s* \( \s*
       " (?P<rhai_function> [^"]+ ) " \s* , \s*
       (?P<rust_function> [a-zA-Z0-9_]+)
    \s* \) \s* ;
    """, re.VERBOSE)
E_REGISTER_FN_STOP = re.compile(r"^}")

DOCCOMMENT_LINE = re.compile(r'^\s*//@\s*(.*)');

E_FN_DECL_START = re.compile(r"""
    ^\s* fn \s*  (?P<rust_fn> [a-zA-Z0-9_]+ ) \s* \(
    """, re.VERBOSE)
E_FN_DECL_PARAM = re.compile(r"""
    (?P<name> [a-zA-Z0-9_]+ ) \s* : \s* (?P<typ> .* ) \s* ,
    """, re.VERBOSE)
E_FN_DECL_END = re.compile(r"""
    \) \s* (?P<ret>  -> (.*) )? { $
    """, re.VERBOSE)
E_FN_DECL_ONELINE = re.compile(r"""
    \b fn \s* 
    (?P<rust_function> [a-zA-Z0-9_]+  )
    \s* \( \s*
        (?P<params> [^)]* )
    \s* \) \s*
    (?P<ret>  -> (.*) )?
    \s* {
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

@dataclass
class TypeAndDoc:
    typ: str
    doc: str

@dataclass
class ExecutorFunc:
    rust_function: str
    rhai_function: str
    primary_doc: str
    params: List[Tuple[str, TypeAndDoc]]
    ret: TypeAndDoc

def read_executor_file(ll: List[str]) -> List[ExecutorFunc]:
    funcs : Dict[str, ExecutorFunc] = {}

    # Step 1: find and parse `pub fn register`

    found = False
    for l in ll:
        if not found:
            if E_REGISTER_FN_START.search(l):
                found=True
        else:
            if E_REGISTER_FN_STOP.search(l):
                break
            if m := E_REGISTER_FN_ENTRY.search(l):
                rs = m.group('rust_function')
                rh = m.group('rhai_function')
                funcs[rs] = ExecutorFunc(
                    rs,rh,'',[],TypeAndDoc('','')
                )

    # Step 2: extract each fuction's documentation, parameters and return type

    accumulated_doccomment_lines : List[str] = []
    active_function : None | str = None
    for l in ll:
        if x := DOCCOMMENT_LINE.search(l):
            accumulated_doccomment_lines.append(x.group(1))
        if active_function is None:
            if x:=E_FN_DECL_ONELINE.search(l):
                g = x.groupdict()
                nam = g["rust_function"]
                if nam in funcs:
                    f = funcs[g["rust_function"]]
                    f.primary_doc = "\n".join(accumulated_doccomment_lines)
                    accumulated_doccomment_lines=[]
                    if "ret" in g:
                        f.ret.typ = g["ret"]
                    params = g["params"]
                    if params:
                        for param in params.split(","):
                            d = param.split(":")
                            f.params.append((d[0].strip(), TypeAndDoc(d[1].strip(),"")))
            elif x:=E_FN_DECL_START.search(l):
                nam = x.group("rust_fn")
                if nam in funcs:
                    active_function = nam
                    funcs[active_function].primary_doc = "\n".join(accumulated_doccomment_lines)
                    accumulated_doccomment_lines=[]
        else:
            if x:=E_FN_DECL_PARAM.search(l):
                nam = x.group("name")
                typ = x.group("typ")
                funcs[active_function].params.append((nam, TypeAndDoc(
                    typ,
                    "\n".join(accumulated_doccomment_lines)
                )))
                accumulated_doccomment_lines=[]
            if x:=E_FN_DECL_END.search(l):
                typ = ""
                if "ret" in x.groupdict():
                    typ =  x.group("ret")
                funcs[active_function].ret = TypeAndDoc(
                    typ,
                    "\n".join(accumulated_doccomment_lines)
                )
                accumulated_doccomment_lines=[]
                active_function=None
            

    return [x for x in funcs.values()]

def strip_handle(s : str) -> str:
    if x:=E_STRIP_HANDLE.search(s):
        s=x.group(1)
    return s.strip()

def document_executor_function(f: ExecutorFunc) -> None:
    print("## " + f.rhai_function)
    print("")
    if f.primary_doc != "":
        print(f.primary_doc)
        print()
    f.params = [x for x in f.params if x[0] != "ctx"]
    if len(f.params) > 0:
        print("Parameters:")
        print()
        for (nam, x) in f.params:
            s = "* " + nam + " (`" + strip_handle(x.typ) + "`)"
            if x.doc:
                s += " - " + x.doc
            print(s)
        print()

    s=""
    if not f.ret.typ:
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

def main() -> None:
    executor_functions : List[ExecutorFunc] = []

    for root, dir, files in os.walk("src/scenario_executor"):
        for fn in files:
            with open(os.path.join(root, fn), "r") as f:
                executor_functions.extend(read_executor_file([line.rstrip() for line in f.readlines()]))

    executor_functions.sort(key=lambda x: x.rhai_function)

    print("# Scenario functions")
    print()
    print("Those functions are used in Weboscat Rhai Scripts (Scenarios):")
    print()

    for execfn in executor_functions:
        document_executor_function(execfn)


if __name__ == '__main__':
    main()
