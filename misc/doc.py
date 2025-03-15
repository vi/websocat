# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "tree-sitter~=0.22.3",
#   "tree-sitter-rust~=0.21.2",
# ]
# ///


import os
import subprocess
import sys

from typing import Generator, Tuple, List, Dict, Set, TypeVar
from dataclasses import dataclass
from tree_sitter import Language, Parser, Tree, Node
from tree_sitter import Query
from collections import defaultdict
import tree_sitter_rust  # type: ignore
import re
from enum import Enum, unique, auto
from pprint import pprint

RUST_LANGUAGE = Language(tree_sitter_rust.language()) # type: ignore

parser = Parser()
parser.language = RUST_LANGUAGE  # type: ignore

DOCCOMMENT_LINE = re.compile(r'^\s*//@\s*(.*)')

E_FN_COOK_RET1 = re.compile(r"""
    \s* -> \s* (.*)
    """, re.VERBOSE)
E_FN_COOK_RET2 = re.compile(r"""
    ^ RhResult \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)
E_STRIP_HANDLE = re.compile(r"""
    ^ Handle \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)

E_METHOD_SELF = re.compile(r"""
    ^ \& \s* mut \s* Handle \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)

############################################################################################

############################################################################################

############################################################################################


@dataclass
class FunctionArg:
    name: str
    doc: List[str]
    typ: str

@dataclass
class RegisterCall:
    rhname: str
    fnname: str

@dataclass
class StructArg:
    name: str
    doc: List[str]
    typ: str

@dataclass
class Callback:
    rettyp: str
    argtyps: List[str]

@dataclass
class Function:
    name: str
    doc: List[str]
    rettyp: str
    retdoc: List[str]
    args: List[FunctionArg]
    reg_calls: List[RegisterCall]
    opts: List[StructArg]
    callbacks : Dict[str,List[Callback]]

@dataclass
class DocumentedIdent:
    ident: str
    doc: List[str]

@dataclass
class ThingWithListOfPrefixes:
    name: str
    prefixes: List[str]

@dataclass
class Outline:
    functions: List[Function]
    endpoints: List[DocumentedIdent]
    overlays: List[DocumentedIdent]
    endpoint_prefixes: List[ThingWithListOfPrefixes]
    overlay_prefixes: List[ThingWithListOfPrefixes]

############################################################################################

############################################################################################

############################################################################################

def qq(x: str) -> Query:
    return Query(RUST_LANGUAGE, x)


REGFN_QUERY = qq('''
(
  call_expression 
  	function:_ @fnc
    (#match? @fnc "engine.register_fn")
    arguments: (arguments(
    	(string_literal (string_content)@rhaifn)
        (identifier) @rustfn
        ))
) @reg
''')

CB1_QUERY = qq('''(
  call_expression 
  	function: (generic_function
        function:(identifier)@fnc
        type_arguments:(type_arguments
        	(_)@params
        )
    )
    arguments: (arguments
      (_)
      (_)@continuation_name
      (_)
    )
    (#eq? @fnc "callback_and_continue")
)''')

CB2_QUERY = qq('''(
  call_expression 
  function: (generic_function
  	function: _@fnc
    (#matches? @fnc "\.callback$")
    type_arguments: (type_arguments . (_)@ret (tuple_type)@params)
  )
    arguments: (arguments
      (_)@continuation_name
      (_)
    )
)''')

OPTS_QUERY = qq('''(
  struct_item  
    name:_@nam
    (#match? @nam "Opts$")
  body: _@content
)''')


FROMSTR_IMPL_QUERY=qq('''(impl_item 
  type:_@typ 
  body:_@content
  (#match? @typ "ParseStrChunkResult")
)''')
# (#match? @typ "ParseStrChunkResult")

STRIP_PREFIX_MANY_QUERY = qq('''(if_expression  
  condition: [
  	(let_condition
    	value: (call_expression
    		function: _@f
            (#match? @f "strip_prefix_many$")
            arguments: (arguments . 
            	(reference_expression
                	value: _@variants
                )
            .)
    	)
  	)
  ]
  consequence: _@content
)''')

STRIP_PREFIX_QUERY = qq('''(if_expression  
  condition: [
  	(let_condition
    	value: (call_expression
    		function: _@f
            
            arguments: (arguments . 
               (string_literal (string_content)@variant)
            .)
    	)
  	)
    (call_expression
        function: _@f
        
        arguments: (arguments . 
        (string_literal (string_content)@variant)
        .)
    )
  ]
  (#match? @f "strip_prefix$|starts_with$")
  consequence: _@content
)''')

LOOKUP_FROMSTR_RESULT_QUERY=qq('''[
    (call_expression 
      function:(scoped_identifier)@variant
      arguments:_@content
     )
    (struct_expression 
    	name:(scoped_type_identifier)@variant
        body:(field_initializer_list
            (field_initializer
              field:_@fieldname
              value:_@content
            )
        )
       (#eq? @fieldname "ovl")
    )
    
    (#match? @variant "^ParseStrChunkResult::Endpoint$|^ParseStrChunkResult::Overlay$")
]''')

FROMSTR_GETNAME_QUERY=qq('''[
    (call_expression 
        function:(scoped_identifier
        path: (identifier)
        name: _@name
    ))
    (scoped_identifier
        path: (identifier)
        name: _@name
    )
    (struct_expression
        name:(scoped_type_identifier
        path: (identifier)
        name: _@name
    ))
]''')

T = TypeVar('T')
def get1match(x: List[Tuple[int, Dict[str, Node]]]) -> None | Dict[str, Node]:
    for xx in x:
        if xx[1]:
            return xx[1]
    return None

def outline(n: Tree) -> Outline:
    a : Node = n.root_node

    funcs : List[Function] = []
    endpoints : List[DocumentedIdent] = []
    overlays: List[DocumentedIdent] = []

    endpoint_prefixes: List[ThingWithListOfPrefixes] = []
    overlay_prefixes: List[ThingWithListOfPrefixes] = []

    doc : List[str] = []

    def process_possible_from_str(a: Node) -> None:
        nonlocal endpoint_prefixes
        nonlocal overlay_prefixes
        if m:=get1match(FROMSTR_IMPL_QUERY.matches(a)):
            def handle_prefixes(prefixes: List[str], content: Node) -> None:
                if m:=get1match(LOOKUP_FROMSTR_RESULT_QUERY.matches(content)):
                    variant=m['variant'].text.decode()
                    content=m['content']

                    name : str = ""
                    if mm:=get1match(FROMSTR_GETNAME_QUERY.matches(content)):
                        name=mm['name'].text.decode()
                    if variant == 'ParseStrChunkResult::Overlay':
                        assert name
                        overlay_prefixes.append(ThingWithListOfPrefixes(name, prefixes))
                    elif variant == 'ParseStrChunkResult::Endpoint':
                        assert name
                        endpoint_prefixes.append(ThingWithListOfPrefixes(name, prefixes))
                    else:
                        print("Something wrong with prefix " + str(prefixes))
                else:
                    print("Error: cannot find match A for prefixes " + str(prefixes))
            for pr in STRIP_PREFIX_MANY_QUERY.matches(m['content']):
                variants=pr[1]['variants']
                content=pr[1]['content']
                prefixes=[t.named_children[0].text.decode() for t in variants.named_children]
                handle_prefixes(prefixes, content)
            for pr in STRIP_PREFIX_QUERY.matches(m['content']):
                variant=pr[1]['variant']
                content=pr[1]['content']
                prefixes=[variant.text.decode()]
                handle_prefixes(prefixes, content)

    process_possible_from_str(a)

    def maybe_doccomment(c : Node) -> bool:
        if c.type == 'line_comment':
            if x:=DOCCOMMENT_LINE.search(c.text.decode()):
                doc.append(x.group(1))
            return True
        return False

    for c in a.children:
        if maybe_doccomment(c): pass
        elif c.type == 'line_comment':
            if x:=DOCCOMMENT_LINE.search(c.text.decode()):
                doc.append(x.group(1))
        elif c.type == 'function_item':
            funcdoc = doc
            doc=[]
            namenode = c.child_by_field_name("name")
            assert namenode
            name = namenode.text.decode()
            rettyp = "()"
            retdoc : List[str] = []

            reg_calls : List[RegisterCall] = []
            struct_opts : List[StructArg] = []
            callbacks : Dict[str,List[Callback]] = defaultdict(list)

            if regfncalls:=REGFN_QUERY.matches(c):
                for regfncall in regfncalls:    
                    rustfnname = regfncall[1]['rustfn'].text.decode()
                    rhaifnname = regfncall[1]['rhaifn'].text.decode()

                    reg_calls.append(RegisterCall(rhaifnname, rustfnname))
            if cb1calls := CB1_QUERY.matches(c):
                for cb1call in cb1calls:
                    params = cb1call[1]['params']

                    argtyps : List[str] = []
                    callback_name = cb1call[1]['continuation_name'].text.decode()
                    rettyp='Handle<Task>'

                    for tuple_node in params.named_children:
                        argtyps.append(tuple_node.text.decode())
                            
                    callbacks[callback_name].append(Callback(rettyp,argtyps))
            if cb2calls := CB2_QUERY.matches(c):
                for cb2call in cb2calls:
                    ret = cb2call[1]['ret']
                    params = cb2call[1]['params']
                    callback_name = cb1call[1]['continuation_name'].text.decode()

                    argtyps = []
                    rettyp= ret.text.decode()

                    for tuple_node in params.named_children:
                        argtyps.append(tuple_node.text.decode())
                            
                    callbacks[callback_name].append(Callback(rettyp,argtyps))
            
            if opts_structs := OPTS_QUERY.matches(c):
                for opts_struct in opts_structs:
                    if 'content' not in opts_struct[1]:
                        continue
                    content = opts_struct[1]['content']
                    doc = []
                    for nn in content.children:
                        if maybe_doccomment(nn): pass
                        elif nn.type == 'field_declaration':
                            name_node = nn.child_by_field_name('name')
                            type_node = nn.child_by_field_name('type')
                            assert name_node
                            assert type_node
                            struct_opts.append(StructArg(name_node.text.decode(), doc, type_node.text.decode()))
                            doc=[]


            for cc in c.children:
                if maybe_doccomment(cc): pass
                elif cc.type=='parameters':
                    doc=[]
                elif cc.type=='type_identifier':
                    retdoc.extend(doc)

            if xx:=c.child_by_field_name("return_type"):
                rettyp = xx.text.decode()
            args : List[FunctionArg] = []

            params = c.child_by_field_name("parameters")
            assert params
            doc=[]
            for cc in params.children:
                if maybe_doccomment(cc): pass
                elif cc.type == 'parameter':
                    pat = cc.child_by_field_name("pattern")
                    assert pat
                    t = cc.child_by_field_name("type")
                    assert t
                    if pat.type=="identifier":
                        pn = pat.text.decode()
                        args.append(FunctionArg(pn,doc,t.text.decode()))
                        doc=[]
            retdoc.extend(doc)

            doc=[]
            funcs.append(Function(name,funcdoc,rettyp, retdoc, args, reg_calls, struct_opts, callbacks))
        elif c.type == 'enum_item':
            name_node = c.child_by_field_name('name')
            assert name_node
            body_node = c.child_by_field_name('body')
            assert body_node
            enumnam = name_node.text.decode()

            if enumnam != 'Endpoint' and enumnam != 'Overlay': continue

            doc=[]
            for cc in body_node.children:
                if maybe_doccomment(cc): pass
                elif cc.type=='enum_variant':
                    variant_name_node=cc.child_by_field_name('name')
                    assert variant_name_node
                    variant_name = variant_name_node.text.decode()
                    if enumnam == 'Endpoint':
                        endpoints.append(DocumentedIdent(variant_name, doc))
                    elif enumnam == 'Overlay':
                        overlays.append(DocumentedIdent(variant_name, doc))
                    doc=[]

    return Outline(funcs, endpoints, overlays, endpoint_prefixes, overlay_prefixes)

def get_merged_outline() -> Outline:
    outlines : List[Outline] = []
    def readfile(fn : str) -> None:
        with open(fn, "rb") as f:
            content : bytes = f.read()
            tree : Tree = parser.parse(content)
            q: Outline = outline(tree)
            outlines.append(q)
    readfile("src/scenario_planner/types.rs")
    readfile("src/scenario_planner/fromstr.rs")

    for root, dir, files in os.walk("src/scenario_executor"):
        for fn in files:
            readfile(os.path.join(root, fn))
    
    functions = [x for xs in outlines for x in xs.functions]
    endpoints = [x for xs in outlines for x in xs.endpoints]
    overlays = [x for xs in outlines for x in xs.overlays]
    endpoint_prefixes = [x for xs in outlines for x in xs.endpoint_prefixes]
    overlay_prefixes = [x for xs in outlines for x in xs.overlay_prefixes]
    return Outline(functions, endpoints, overlays, endpoint_prefixes, overlay_prefixes)

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

    outline : Outline = get_merged_outline()
    
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
