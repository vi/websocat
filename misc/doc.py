# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "tree-sitter~=0.22.3",
#   "tree-sitter-rust~=0.21.2",
# ]
# ///


import os

from typing import Generator, Tuple, List, Dict, Set
from dataclasses import dataclass
from tree_sitter import Language, Parser, Tree, Node
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
    callbacks : List[Callback]

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


def outline(n: Tree) -> Outline:
    a : Node = n.root_node

    funcs : List[Function] = []
    endpoints : List[DocumentedIdent] = []
    overlays: List[DocumentedIdent] = []

    endpoint_prefixes: List[ThingWithListOfPrefixes] = []
    overlay_prefixes: List[ThingWithListOfPrefixes] = []

    doc : List[str] = []

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

            reg_calls : List[RegisterCall] = []
            struct_opts : List[StructArg] = []
            callbacks : List[Callback] = []

            b = c.child_by_field_name('body')
            if b:
                def check_body(n : Node) -> None:
                    nonlocal doc
                    if n.type == 'call_expression':
                        funct_node = n.child_by_field_name('function')
                        args_node = n.child_by_field_name('arguments')
                        assert funct_node
                        assert args_node
                        fff : str = funct_node.text.decode()
                        aaa = n.child_by_field_name('arguments')
                        assert aaa
                        argtyps : List[str] = []
                        if fff == 'engine.register_fn' and len(aaa.named_children)==2:
                            rhname = args_node.named_children[0].named_children[0].text.decode()
                            fnname = args_node.named_children[1].text.decode()
                            reg_calls.append(RegisterCall(rhname, fnname))
                        elif "callback_and_continue" in fff and funct_node.type=='generic_function':
                            typargs_node = funct_node.child_by_field_name('type_arguments')
                            assert typargs_node
                            rettyp='Handle<Task>'

                            for tuple_node in typargs_node.named_children[0].named_children:
                                argtyps.append(tuple_node.text.decode())
                            
                            callbacks.append(Callback(rettyp,argtyps))
                        elif "callback" in fff and funct_node.type=='generic_function':
                            typargs_node = funct_node.child_by_field_name('type_arguments')
                            assert typargs_node
                            rettyp=typargs_node.named_children[0].text.decode()

                            for tuple_node in typargs_node.named_children[1].named_children:
                                argtyps.append(tuple_node.text.decode())
                            
                            callbacks.append(Callback(rettyp,argtyps))
                            
                    elif n.type == 'struct_item':
                        name_node = n.child_by_field_name('name')
                        body_node = n.child_by_field_name('body')
                        assert name_node
                        assert body_node
                        name : str = name_node.text.decode()
                        if name.endswith("Opts"):
                            doc = []
                            for nn in body_node.children:
                                if maybe_doccomment(nn): pass
                                elif nn.type == 'field_declaration':
                                    name_node = nn.child_by_field_name('name')
                                    type_node = nn.child_by_field_name('type')
                                    assert name_node
                                    assert type_node
                                    struct_opts.append(StructArg(name_node.text.decode(), doc, type_node.text.decode()))
                                    doc=[]
                    for nc in n.named_children:
                        check_body(nc)
                check_body(b)
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
        elif c.type == 'impl_item':
            type_node = c.child_by_field_name('type')
            assert type_node
            body_node = c.child_by_field_name('body')
            assert body_node
            typename = type_node.text.decode()
            
            if not typename.startswith('ParseStrChunkResult'): continue

            prefixes : List[str] = []
            def check_body(n : Node) -> None:
                nonlocal prefixes
                if n.type == 'call_expression':
                    funct_node = n.child_by_field_name('function')
                    args_node = n.child_by_field_name('arguments')
                    assert funct_node
                    assert args_node
                    fff : str = funct_node.text.decode()
                    aaa = n.child_by_field_name('arguments')
                    assert aaa
                    argtyps : List[str] = []
                    if (fff.endswith('.starts_with') or fff.endswith('.strip_prefix')) and len(aaa.named_children)==1:
                        prefix = args_node.named_children[0].named_children[0].text.decode()
                        prefixes.append(prefix)
                    elif "strip_prefix_many" in fff:
                        for prefix_node in aaa.named_children[0].named_children[0].named_children:
                            assert prefix_node.type == 'string_literal'
                            prefixes.append(prefix_node.named_children[0].text.decode())
                    elif fff == "ParseStrChunkResult::Endpoint":
                        inner = aaa.named_children[0]
                        endpoint_name : str
                        match inner.type:
                            case 'call_expression':
                                endpoint_name = inner.named_children[0].named_children[1].text.decode()
                            case 'struct_expression':
                                endpoint_name = inner.named_children[0].named_children[1].text.decode()
                            case 'scoped_identifier':
                                endpoint_name = inner.named_children[1].text.decode()
                            case _: 
                                print(inner.type)
                                assert False
                        if prefixes:
                            endpoint_prefixes.append(ThingWithListOfPrefixes(endpoint_name, prefixes))
                            prefixes=[]
                elif n.type == 'struct_expression':
                    name_node = n.child_by_field_name('name')
                    assert name_node
                    name = name_node.text.decode()
                    body_node = n.child_by_field_name('body')
                    if name == 'ParseStrChunkResult::Overlay':
                        assert body_node
                        for nn in body_node.named_children:
                            if nn.type != 'field_initializer': continue
                            field_node = nn.child_by_field_name('field')
                            assert field_node
                            if field_node.text.decode() != 'ovl': continue
                            value_node = nn.child_by_field_name('value')
                            assert value_node
                            overlay_name : str
                            if value_node.type=='struct_expression':
                                overlay_name = value_node.named_children[0].named_children[1].text.decode()
                            elif value_node.type == 'call_expression':
                                overlay_name = value_node.named_children[0].named_children[1].text.decode()
                            elif value_node.type == 'scoped_identifier':
                                overlay_name = value_node.named_children[1].text.decode()
                            else:
                                assert False
                            if prefixes:
                                overlay_prefixes.append(ThingWithListOfPrefixes(overlay_name, prefixes))
                                prefixes=[]
                            
                    
                for nc in n.named_children:
                    check_body(nc)
            check_body(body_node)

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
class ExecutorFunc:
    rust_function: str
    rhai_function: str
    primary_doc: str
    params: List[Tuple[str, TypeAndDoc]]
    ret: TypeAndDoc
    callback_params: List[str]
    callback_return: str
    options: List[Tuple[str, TypeAndDoc]]

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

    print()
    print("## Endpoints")
    print()

    for ep in c.endpoints:
        document_item(ep, "endpoint")

    print()
    print("## Overlays")
    print()
    
    for ep in c.overlays:
        document_item(ep, "overlay")

    pass

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
            cbparams : List[str] = []
            cbret : str = ""
            if f.callbacks:
                cbret = f.callbacks[0].rettyp
                cbparams = f.callbacks[0].argtyps
            funcs.append(ExecutorFunc(
                f.name,
                approved_functitons[f.name],
                "\n".join(f.doc),
                [ (x.name, TypeAndDoc(x.typ, " ".join(x.doc))) for x in f.args  ],
                TypeAndDoc(f.rettyp, " ".join(f.retdoc)),
                cbparams,
                cbret,
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

    document_planner_content(planner_content)

    print("# Scenario functions")
    print()
    print("Those functions are used in Websocat Rhai Scripts (Scenarios):")
    print()

    for execfn in executor_functions:
        document_executor_function(execfn)

    
    print(r"""
# Glossary

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


if __name__ == '__main__':
    main()
