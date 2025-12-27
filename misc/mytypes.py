from typing import Generator, Tuple, List, Dict, Set, TypeVar, Self, Any
from dataclasses import dataclass
from dataclasses_json import dataclass_json

@dataclass_json
@dataclass
class FunctionArg:
    name: str
    doc: List[str]
    typ: str

@dataclass_json
@dataclass
class RegisterCall:
    rhname: str
    fnname: str

@dataclass_json
@dataclass
class StructArg:
    name: str
    doc: List[str]
    typ: str

@dataclass_json
@dataclass
class Callback:
    rettyp: str
    argtyps: List[str]

@dataclass_json
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

@dataclass_json
@dataclass
class DocumentedIdent:
    ident: str
    doc: List[str]

@dataclass_json
@dataclass
class ThingWithListOfPrefixes:
    name: str
    prefixes: List[str]

@dataclass_json
@dataclass
class Outline:
    functions: List[Function]
    endpoints: List[DocumentedIdent]
    overlays: List[DocumentedIdent]
    endpoint_prefixes: List[ThingWithListOfPrefixes]
    overlay_prefixes: List[ThingWithListOfPrefixes]


@dataclass_json
@dataclass
class NameTypeAndDoc:
    nam: str
    typ: str
    doc: str

@dataclass_json
@dataclass
class ExecutorFuncCallback:
    params: List[str]
    rettyp: str

@dataclass_json
@dataclass
class ExecutorFunc:
    rust_function: str
    rhai_function: str
    primary_doc: str
    params: List[NameTypeAndDoc]
    ret: NameTypeAndDoc
    callbacks: Dict[str, ExecutorFuncCallback]
    options: List[NameTypeAndDoc]

@dataclass_json    
@dataclass
class PlannerItem:
    name: str
    prefixes: List[str]
    doc: str

@dataclass_json
@dataclass
class PlannerContent:
    endpoints: List[PlannerItem]
    overlays: List[PlannerItem]

@dataclass_json
@dataclass
class ThingsToDocument:
    planner_content: PlannerContent
    executor_functions : List[ExecutorFunc]

    def to_json(self) -> str: raise Exception("This method should be overridden by @dataclass_json")
    def from_json(s: str|Any) -> Self: raise Exception("This method should be overridden by @dataclass_json")
