from typing import Generator, Tuple, List, Dict, Set, TypeVar
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
