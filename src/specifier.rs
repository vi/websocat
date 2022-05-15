use super::{L2rUser, Options, Result};
use super::{PeerConstructor, ProgramState};
use std;
use std::cell::RefCell;
use std::rc::Rc;

pub enum ClassMessageBoundaryStatus {
    StreamOriented,
    MessageOriented,
    MessageBoundaryStatusDependsOnInnerType,
}

pub enum ClassMulticonnectStatus {
    MultiConnect,
    SingleConnect,
    MulticonnectnessDependsOnInnerType,
}

/// A trait for a each specified type's accompanying object
///
/// Don't forget to register each instance at the `list_of_all_specifier_classes` macro.
pub trait SpecifierClass: std::fmt::Debug {
    /// The primary name of the class
    fn get_name(&self) -> &'static str;
    /// Names to match command line parameters against, with a `:` colon if needed
    fn get_prefixes(&self) -> Vec<&'static str>;
    /// --long-help snippet about this specifier
    fn help(&self) -> &'static str;
    /// Given the command line text, construct the specifier
    /// arg is what comes after the colon (e.g. `//echo.websocket.org` in `ws://echo.websocket.org`)
    fn construct(&self, arg: &str) -> Result<Rc<dyn Specifier>>;
    /// Given the inner specifier, construct this specifier.
    fn construct_overlay(&self, inner: Rc<dyn Specifier>) -> Result<Rc<dyn Specifier>>;
    /// Returns if this specifier is an overlay
    fn is_overlay(&self) -> bool;
    /// True if it is not expected to preserve message boundaries on reads
    fn message_boundary_status(&self) -> ClassMessageBoundaryStatus;

    fn multiconnect_status(&self) -> ClassMulticonnectStatus;
    /// If it is Some then is_overlay, construct and most other things are ignored and prefix get replaced...
    fn alias_info(&self) -> Option<&'static str>;
}

macro_rules! specifier_alias {
    (name=$n:ident,
            prefixes=[$($p:expr),*],
            alias=$x:expr,
            help=$h:expr) => {
        #[derive(Debug,Default)]
        pub struct $n;
        impl $crate::SpecifierClass for $n {
            fn get_name(&self) -> &'static str { stringify!($n) }
            fn get_prefixes(&self) -> Vec<&'static str> { vec![$($p),*] }
            fn help(&self) -> &'static str { $h }
            fn message_boundary_status(&self) -> $crate::ClassMessageBoundaryStatus {
                panic!("Error: message_boundary_status called on alias class")
            }
            fn multiconnect_status(&self) -> $crate::ClassMulticonnectStatus {
                panic!("Error: multiconnect_status called on alias class")
            }
            fn is_overlay(&self) -> bool {
                false
            }
            fn construct(&self, _arg:&str) -> $crate::Result<Rc<dyn Specifier>> {
                panic!("Error: construct called on alias class")
            }
            fn construct_overlay(&self, _inner : Rc<dyn Specifier>) -> $crate::Result<Rc<dyn Specifier>> {
                panic!("Error: construct_overlay called on alias class")
            }
            fn alias_info(&self) -> Option<&'static str> { Some($x) }
        }
    };
}

macro_rules! specifier_class {
    (name=$n:ident,
            target=$t:ident,
            prefixes=[$($p:expr),*],
            arg_handling=$c:tt,
            overlay=$o:expr,
            $so:expr,
            $ms:expr,
            help=$h:expr) => {
        #[derive(Debug,Default)]
        pub struct $n;
        impl $crate::SpecifierClass for $n {
            fn get_name(&self) -> &'static str { stringify!($n) }
            fn get_prefixes(&self) -> Vec<&'static str> { vec![$($p),*] }
            fn help(&self) -> &'static str { $h }
            fn message_boundary_status(&self) -> $crate::ClassMessageBoundaryStatus {
                use $crate::ClassMessageBoundaryStatus::*;
                $so
            }
            fn multiconnect_status(&self) -> $crate::ClassMulticonnectStatus {
                use $crate::ClassMulticonnectStatus::*;
                $ms
            }
            fn is_overlay(&self) -> bool {
                $o
            }
            specifier_class!(construct target=$t $c);
        }
    };
    (construct target=$t:ident noarg) => {
        fn construct(&self, just_arg:&str) -> $crate::Result<Rc<dyn Specifier>> {
            if just_arg != "" {
                Err(format!("{}-specifer requires no parameters. `{}` is not needed",
                    self.get_name(), just_arg))?;
            }
            Ok(Rc::new($t))
        }
        fn construct_overlay(&self, _inner : Rc<dyn Specifier>) -> $crate::Result<Rc<dyn Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
        fn alias_info(&self) -> Option<&'static str> { None }
    };
    (construct target=$t:ident into) => {
        fn construct(&self, just_arg:&str) -> $crate::Result<Rc<dyn Specifier>> {
            Ok(Rc::new($t(just_arg.into())))
        }
        fn construct_overlay(&self, _inner : Rc<dyn Specifier>) -> $crate::Result<Rc<dyn Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
        fn alias_info(&self) -> Option<&'static str> { None }
    };
    (construct target=$t:ident parse) => {
        fn construct(&self, just_arg:&str) -> $crate::Result<Rc<dyn Specifier>> {
            Ok(Rc::new($t(just_arg.parse()?)))
        }
        fn construct_overlay(&self, _inner : Rc<dyn Specifier>) -> $crate::Result<Rc<dyn Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
        fn alias_info(&self) -> Option<&'static str> { None }
    };
    (construct target=$t:ident parseresolve) => {
        fn construct(&self, just_arg:&str) -> $crate::Result<Rc<dyn Specifier>> {
            use std::net::ToSocketAddrs;
            info!("Resolving hostname to IP addresses");
            let addrs : Vec<std::net::SocketAddr> = just_arg.to_socket_addrs()?.collect();
            if addrs.is_empty() {
                Err("Failed to resolve this hostname to IP")?;
            }
            for addr in &addrs {
                info!("Got IP: {}", addr);
            }
            Ok(Rc::new($t(addrs)))
        }
        fn construct_overlay(&self, _inner : Rc<dyn Specifier>) -> $crate::Result<Rc<dyn Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
        fn alias_info(&self) -> Option<&'static str> { None }
    };
    (construct target=$t:ident subspec) => {
        fn construct(&self, just_arg:&str) -> $crate::Result<Rc<dyn Specifier>> {
            Ok(Rc::new($t($crate::spec(just_arg)?)))
        }
        fn construct_overlay(&self, _inner : Rc<dyn Specifier>) -> $crate::Result<Rc<dyn Specifier>> {
            Ok(Rc::new($t(_inner)))
        }
        fn alias_info(&self) -> Option<&'static str> { None }
    };
    (construct target=$t:ident {$($x:tt)*}) => {
        $($x)*
        fn alias_info(&self) -> Option<&'static str> { None }
    };
}


#[derive(Debug)]
pub struct SpecifierNode {
    pub cls: Rc<dyn SpecifierClass>,
    //pub opt: Rc<std::any::Any>,
}

#[derive(Debug)]
pub struct SpecifierStack {
    pub addr: String,
    pub addrtype: SpecifierNode,
    pub overlays: Vec<SpecifierNode>,
}

#[derive(Clone)]
pub struct ConstructParams {
    pub global_state: Rc<RefCell<ProgramState>>,
    pub program_options: Rc<Options>,
    pub left_to_right: L2rUser,
}

/// All of those methods are about left_to_right mechanism
impl ConstructParams {
    /// Reset left_to_right to default value.
    pub fn reset_l2r(&mut self) {
        match self.left_to_right {
            L2rUser::FillIn(ref mut x) => {
                *x.borrow_mut() = Default::default();
                //*x = Rc::new(RefCell::new(Default::default()));
            }
            L2rUser::ReadFrom(_) => panic!("ConstructParams::reset_l2r called wrong"),
        }
    }
    /// Clones ConstructParams, changing FillIn to ReadFrom in left_to_right field
    /// and also disassociating it from the original RefCell.
    ///
    /// Panics when called on object with left_to_right set to ReadFrom.
    pub fn reply(&self) -> Self {
        let l2r = match self.left_to_right {
            L2rUser::FillIn(ref x) => Rc::new(x.borrow().clone()),
            L2rUser::ReadFrom(_) => panic!("ConstructParams::reply called wrong"),
        };
        ConstructParams {
            global_state: self.global_state.clone(),
            program_options: self.program_options.clone(),
            left_to_right: L2rUser::ReadFrom(l2r),
        }
    }

    pub fn deep_clone(&self) -> Self {
        let l2r = match self.left_to_right {
            L2rUser::FillIn(ref x) => L2rUser::FillIn(Rc::new(RefCell::new(x.borrow().clone()))),
            L2rUser::ReadFrom(_) => {
                panic!(
                    "You are not supposed to use ConstructParams::deep_clone on ReadFrom things"
                );
            }
        };
        ConstructParams {
            global_state: self.global_state.clone(),
            program_options: self.program_options.clone(),
            left_to_right: l2r,
        }
    }

    /// Access specified-specific global (singleton) data
    pub fn global<T:std::any::Any, F>(&self, def:F) -> std::cell::RefMut<T> 
        where F : FnOnce()->T
    {
        std::cell::RefMut::map(
            self.global_state.borrow_mut(),
            |x|{
                x.0.entry::<T>().or_insert_with(def)
            }
        )
    }
}

/// A parsed command line argument.
/// For example, `ws-listen:tcp-l:127.0.0.1:8080` gets parsed into
/// a `WsUpgrade(TcpListen(SocketAddr))`.
pub trait Specifier: std::fmt::Debug {
    /// Apply the specifier for constructing a "socket" or other connecting device.
    fn construct(&self, p: ConstructParams) -> PeerConstructor;

    // Specified by `specifier_boilerplate!`:
    fn is_multiconnect(&self) -> bool;
    fn uses_global_state(&self) -> bool;
}

impl Specifier for Rc<dyn Specifier> {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        (**self).construct(p)
    }

    fn is_multiconnect(&self) -> bool {
        (**self).is_multiconnect()
    }
    fn uses_global_state(&self) -> bool {
        (**self).uses_global_state()
    }
}

macro_rules! specifier_boilerplate {
    (singleconnect $($e:tt)*) => {
        fn is_multiconnect(&self) -> bool { false }
        specifier_boilerplate!($($e)*);
    };
    (multiconnect $($e:tt)*) => {
        fn is_multiconnect(&self) -> bool { true }
        specifier_boilerplate!($($e)*);
    };
    (no_subspec $($e:tt)*) => {
        specifier_boilerplate!($($e)*);
    };
    (has_subspec $($e:tt)*) => {
        specifier_boilerplate!($($e)*);
    };
    () => {
    };
    (globalstate $($e:tt)*) => {
        fn uses_global_state(&self) -> bool { true }
        specifier_boilerplate!($($e)*);
    };
    (noglobalstate $($e:tt)*) => {
        fn uses_global_state(&self) -> bool { false }
        specifier_boilerplate!($($e)*);
    };
}

macro_rules! self_0_is_subspecifier {
    (...) => {
       // removed with old linter
    };
    (proxy_is_multiconnect) => {
        self_0_is_subspecifier!(...);
        fn is_multiconnect(&self) -> bool { self.0.is_multiconnect() }
    };
}
