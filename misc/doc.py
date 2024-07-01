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
E_FN_BODY_CALLBACK1 = re.compile(r"""
   callback_and_continue \s* ::< \s* \(
    (?P<cbparams>  .* )
    \s* \) \s* > \s* \(
    """, re.VERBOSE)
E_FN_BODY_CALLBACK2 = re.compile(r"""
   callback \s* ::< 
       (?P<cbret> [^,]+? )
    \s* \, \s* \(
       (?P<cbparams>  .* )
    \s* \) \s* > \s* \(
    """, re.VERBOSE)
E_FN_OPTS_START = re.compile(r"""
   ^ \s* struct \s* [a-zA-Z0-9_]+ \s* { \s* $
    """, re.VERBOSE)
E_FN_OPTS_ENTRY = re.compile(r"""
   ^ \s* 
        (?P<nam> [a-zA-Z0-9_]+ )
   \s* : \s*  
        (?P<typ>  [^,]+  )
   \s* , \s* $
    """, re.VERBOSE)
E_FN_OPTS_END = re.compile(r"""
   ^ \s* } \s* $
    """, re.VERBOSE)

E_FN_BODY_END = re.compile('^}\s*$')


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
    callback_params: List[str]
    callback_return: str
    options: List[Tuple[str, TypeAndDoc]]

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
                    rs,rh,'',[],TypeAndDoc('',''),[],"",[]
                )

    # Step 2: extract each fuction's documentation, parameters and return type

    accumulated_doccomment_lines : List[str] = []
    active_function_decl : None | str = None
    active_function_body: None | str = None
    active_options_list = False

    def take_comment() -> str:
        nonlocal accumulated_doccomment_lines
        ret = "\n".join(accumulated_doccomment_lines)
        accumulated_doccomment_lines=[]
        return ret

    for l in ll:
        if x := DOCCOMMENT_LINE.search(l):
            accumulated_doccomment_lines.append(x.group(1))
        if active_function_decl is None:
            if active_function_body is None:
                if x:=E_FN_DECL_ONELINE.search(l):
                    g = x.groupdict()
                    nam = g["rust_function"]
                    if nam in funcs:
                        f = funcs[g["rust_function"]]
                        f.primary_doc = take_comment()
                        if "ret" in g:
                            f.ret.typ = g["ret"]
                        params = g["params"]
                        if params:
                            for param in params.split(","):
                                d = param.split(":")
                                f.params.append((d[0].strip(), TypeAndDoc(d[1].strip(),"")))
                        active_function_body = nam
                elif x:=E_FN_DECL_START.search(l):
                    nam = x.group("rust_fn")
                    if nam in funcs:
                        active_function_decl = nam
                        funcs[active_function_decl].primary_doc = take_comment()
                        active_function_body = nam
            else:
                # inside a function body
                if not active_options_list:
                    def process_cbparams(cbp: str, ret:str) -> None:
                        if active_function_body is None: raise Exception("!")
                        if len(funcs[active_function_body].callback_params) == 0:
                            for p in cbp.split(","):
                                if p.strip() == "": continue
                                funcs[active_function_body].callback_params.append(p)
                        if funcs[active_function_body].callback_return == "":
                            funcs[active_function_body].callback_return = ret
                    if E_FN_BODY_END.search(l):
                        active_function_body = None
                    elif x:= E_FN_BODY_CALLBACK1.search(l):
                        process_cbparams(x.group("cbparams"), "Task")
                    elif x:= E_FN_BODY_CALLBACK2.search(l):
                        process_cbparams(x.group("cbparams"), x.group("cbret"))
                    elif E_FN_OPTS_START.search(l):
                        active_options_list = True
                else:
                    # inside options list
                    if E_FN_OPTS_END.search(l):
                        active_options_list = False
                    elif x := E_FN_OPTS_ENTRY.search(l):
                        nam = x.group("nam")
                        typ = x.group("typ")
                        doc = take_comment()
                        funcs[active_function_body].options.append((nam, TypeAndDoc(typ, doc)))
        else:
            # inside a function declaration
            if x:=E_FN_DECL_PARAM.search(l):
                nam = x.group("name")
                typ = x.group("typ")
                funcs[active_function_decl].params.append((nam, TypeAndDoc(
                    typ,
                    take_comment(),
                )))
            if x:=E_FN_DECL_END.search(l):
                typ = ""
                if "ret" in x.groupdict():
                    typ =  x.group("ret")
                funcs[active_function_decl].ret = TypeAndDoc(
                    typ,
                    take_comment(),
                )
                active_function_decl=None
            

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
            if nam == "opts" and x.typ == "Dynamic" and not x.doc:
                x.doc = "object map containing dynamic options to the function"
            s = "* " + nam + " (`" 
            if x.typ != "FnPtr":
                s += strip_handle(x.typ)
            else:
                s += "Fn("
                for (i,pt) in enumerate(f.callback_params):
                    if i>0:
                        s += ", "
                    s += strip_handle(pt.strip())
                s += ")"
                if f.callback_return:
                    s += " -> "
                    s += strip_handle(f.callback_return)
                if not x.doc:
                    x.doc = "Rhai function that will be called to continue processing"
            s += "`)"
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
    if len(f.options) > 0:
        print("Options:")
        print()
        for (on, od) in f.options:
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

def read_planner_data(planner_types : List[str], planner_fromstr : List[str]) -> PlannerContent:
    c = PlannerContent([], [])
    return c


def document_planner_content(c: PlannerContent) -> None:
    pass

############################################################################################

############################################################################################

############################################################################################

def main() -> None:
    executor_functions : List[ExecutorFunc] = []

    planner_types : List[str]
    planner_fromstr : List[str]
    with open("src/scenario_planner/types.rs", "r") as f:
        planner_types = [x for x in f.readlines()]
    with open("src/scenario_planner/fromstr.rs", "r") as f:
        planner_fromstr = [x for x in f.readlines()]

    planner_content = read_planner_data(planner_types, planner_fromstr)

    for root, dir, files in os.walk("src/scenario_executor"):
        for fn in files:
            with open(os.path.join(root, fn), "r") as f:
                executor_functions.extend(read_executor_file([line.rstrip() for line in f.readlines()]))

    executor_functions.sort(key=lambda x: x.rhai_function)

    print(r"""# Glossary

* Specifier - WebSocket URL, TCP socket address or other connection type Websocat recognizes, 
or an overlay that transforms other Specifier.
* Endpoint - leaf-level specifier that directly creates some sort of Socket
* Overlay - intermediate specifier that transforms inner specifier
* Socket - a pair of byte stream- or datagram-oriented data flows: write 
(to socket) and read (from socket), optionally with a hangup signal
* Scenario = Websocat Rhai Script - detailed instruction of how Websocat would perform its operation.
Normally it is generated automatically from CLI arguments, then executed; but you can separate 
those steps and customize the scenario to fine tune of how Websocat operates. Just like usual CLI API, 
Rhai functions API is also intended to be semver-stable API of Websocat.
* Scenario function - Rhai native function that Websocat registers with Rhai engine that can be used 
in Scenarios.
* Scenario Planner - part of Websocat implementation that parses command line arguments and prepares a Scenario
* Scenario Executor - part of Websocat implementation that executes a Scenario.
* CLI arguments - combination of a positional arguments (typically Specifiers) and various 
flags (e.g. `--binary`) and options (e.g. `--buffer-size 4096`) that affect Scenario Planner.

""")



    print("# CLI specifiers")
    print()
    print("Things you can use as a (part of a) positional command-line argument in Websocat")
    print()

    document_planner_content(planner_content)

    print("# Scenario functions")
    print()
    print("Those functions are used in Websocat Rhai Scripts (Scenarios):")
    print()

    for execfn in executor_functions:
        document_executor_function(execfn)


if __name__ == '__main__':
    main()
