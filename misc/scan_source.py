import os
import subprocess
import sys

from typing import Generator, Tuple, List, Dict, Set, TypeVar
from tree_sitter import Language, Parser, Tree, Node
from tree_sitter import Query
from collections import defaultdict
import tree_sitter_rust  # type: ignore
import re
from enum import Enum, unique, auto
from pprint import pprint

from mytypes import *

import json

RUST_LANGUAGE = Language(tree_sitter_rust.language()) # type: ignore

parser = Parser()
parser.language = RUST_LANGUAGE  # type: ignore

DOCCOMMENT_LINE = re.compile(r'^\s*//@\s*(.*)')


E_METHOD_SELF = re.compile(r"""
    ^ \& \s* mut \s* Handle \s* < \s* (.*) \s* > \s* $
    """, re.VERBOSE)



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
    (#matches? @fnc "\\.callback$")
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
    (#not-match? @content "Endpoint::TcpConnectByLateHostname")
]''')

FROMSTR_GETNAME_QUERY=qq('''[
    (call_expression 
        function:(scoped_identifier
            path: (identifier)@_path
            name: _@name
        )
    )
    (scoped_identifier
        path: (identifier)@_path
        name: _@name
    )
    (struct_expression
        name:(scoped_type_identifier
            path: (identifier)@_path
            name: _@name
        )
    )
    (#match? @_path "^Overlay$|^Endpoint$")
]''')

def debugprint_query_result(x: List[Tuple[int, Dict[str, Node | list[Node]]]]):
    skipped = 0
    for (i, xx) in x:
        if not xx:
            skipped+=1
            continue
        print("======", i)
        for (k,v) in xx.items():
            print("---")
            print(k)
            if(isinstance(v,Node)):
                print(v)
                print(v.text)
            elif(isinstance(v,List[Node])):
                for vv in v:
                    print("...")
                    print(vv)
                    print(vv.text)
    print("skipped", skipped)


T = TypeVar('T')
def get1match(x: List[Tuple[int, Dict[str, Node | list[Node]]]]) -> None | Dict[str, Node]:
    candidate : Dict[str, Node | list[Node]] | None = None
    for xx in x:
        if xx[1]:
            if candidate is None:
                candidate = xx[1]
            else:
                new_candidate = xx[1]
                remove_underscored_fields_from_dict = lambda d: dict(filter(lambda item: not item[0].startswith("_"),d.items()))
                cand_f = remove_underscored_fields_from_dict(candidate)
                newcand_f = remove_underscored_fields_from_dict(new_candidate)
                if cand_f != newcand_f:
                    debugprint_query_result(x)
                    raise Exception("Multiple matches where one expected")

    if candidate is None:
        return None
   
    for (k,v) in candidate.items():
        if(not(isinstance(v,Node))):
            debugprint_query_result(x)
            raise Exception("List of nodes returned where just one node expected")
    return candidate  # ty:ignore[invalid-return-type]

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
            def handle_prefixes(prefixes: List[str], content: Node|List[Node]) -> None:
                if not isinstance(content, Node):
                    raise Exception("Unexpected list of nodes")
                if m:=get1match(LOOKUP_FROMSTR_RESULT_QUERY.matches(content)):
                    variant=m['variant'].text.decode()
                    content : Node = m['content']

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


def process_outline(o: Outline) -> ThingsToDocument:
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
            cbmap : Dict[str,ExecutorFuncCallback] = {}
            for (k, v) in f.callbacks.items():
                cbmap[k] = ExecutorFuncCallback(v[0].argtyps, v[0].rettyp)
            params = [ NameTypeAndDoc(x.name, x.typ, " ".join(x.doc)) for x in f.args  ]
            params = [x for x in params if x.nam != "ctx"]
            displayname = approved_functitons[f.name]

            if params:
                firstparam = params[0]
                if mn := E_METHOD_SELF.search(firstparam.typ):
                    x = mn.group(1)
                    params.pop(0)
                    displayname=x + "::" + displayname

            funcs.append(ExecutorFunc(
                f.name,
                displayname,
                "\n".join(f.doc),
                params,
                NameTypeAndDoc("ret", f.rettyp, " ".join(f.retdoc)),
                cbmap,
                [NameTypeAndDoc(x.name, x.typ, " ".join(x.doc)) for x in f.opts]
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


    return ThingsToDocument(PlannerContent(endpoints, overlays), funcs)



def main() -> None:
    outline : Outline = get_merged_outline()
    things : ThingsToDocument = process_outline(outline)
    
    print(things.to_json())  # ty:ignore[unresolved-attribute]

if __name__ == '__main__':
    main()
