fn q () -> string_interner::StringInterner<string_interner::symbol::SymbolU16, string_interner::backend::StringBackend  <string_interner::symbol::SymbolU16>> {
    let mut s = string_interner::StringInterner::new();
    s.get_or_intern("qqq1");
    s.get_or_intern("qqq2");
    s.get_or_intern("qqq3");
    s.get_or_intern("qqq4");
    s.get_or_intern("qqq5");
    s.get_or_intern("qqq6");
    s.get_or_intern("qqq7");
    s
}

fn main() {
    let sym1;
    let sym2;
    let sym3;
    let sym4;
    {
        let s = q();
        sym1 = s.get("qqq2").unwrap();
        sym2 = s.get("qqq7").unwrap();
        sym3 = s.get("qqq3").unwrap();
        sym4 = s.get("qqq5").unwrap();
    }
    println!("{:?} {:?} {:?} {:?}", sym1, sym2, sym3, sym4);

    {
        let s = q();
        println!("{}", s.resolve(sym1).unwrap());
        println!("{}", s.resolve(sym2).unwrap());
        println!("{}", s.resolve(sym3).unwrap());
        println!("{}", s.resolve(sym4).unwrap());

    }

    {
        use string_interner::Symbol;
        let mut s: string_interner::StringInterner = string_interner::StringInterner::with_capacity(5usize);
        assert_eq!(
            s.get_or_intern("hoo"),
            ::string_interner::DefaultSymbol::try_from_usize(0usize).unwrap()
        );
        assert_eq!(
           s.get_or_intern("loo"),
           ::string_interner::DefaultSymbol::try_from_usize(1usize).unwrap()
       );
       assert_eq!(
           s.get_or_intern("aoo"),
           ::string_interner::DefaultSymbol::try_from_usize(2usize).unwrap()
       );
       assert_eq!(
           s.get_or_intern("jjj2"),
           ::string_interner::DefaultSymbol::try_from_usize(3usize).unwrap()
       );
       assert_eq!(
           s.get_or_intern("phh"),
           ::string_interner::DefaultSymbol::try_from_usize(4usize).unwrap()
       );
    }
}
