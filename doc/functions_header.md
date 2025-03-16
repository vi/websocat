# Scenario functions

Prior to doing any network things, Websocat prepares a Scenario (Websocat Rhai Script) based on you command line options.
Scenarios are less stable than usual Websocat API, but allow fine tuning Websocat behaviour.
You can view scenarios using `--dump-spec` option and execute them the with `-x` option.

In this chapter, each entry corresponds to one native Rhai function.
If heading begins with `Something::`, it means this is a method to be called on objects of the type Something.

Often functions take a Rhai object map (`#{...}`) and Rhai closures `|| { ... }` as parameters. Map elements and closure parameters are also documented.

Note that types here are rendered as Rush types, but it should be straightforward to deduce Rhai things to use using common sense.

The following functions and methods are used in scenarios:
