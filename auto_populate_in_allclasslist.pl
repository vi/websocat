#!/usr/bin/perl
use v5.10;
use strict;
use warnings;
use utf8;
use open qw(:std :utf8);


# A script to scan Websocat3's source code and find all classes and macroses, writing the `websocat-allnodes` crate using code generation.
# `websocat-allnodes` is intended to be tracked in Git, to avoid having this hacky script as a build step
# This script does not use `syn` crate or something like that, it just parses `-Zunpretty=expanded` output using regexes and
# relies on display details like indentation to track when we are entering and leaving modules
# Now the script works with rustc 1.60.0.
# Classes and macros are found by scanning for a special attribute. For macros the "attribute" is placed in a doccomment though, as they don't use derive macro for now.

$_=`rg -l auto_populate_in_allclasslist | rg crates | rg -v 'allnodes|derive' | cut -f 2-2 -d/ | sort -u`;
our @crates_to_scan = split '\n';


open F, ">", "crates/websocat-allnodes/Cargo.toml";

print F <<END
# Note: this is an auto-generated file, but is intended to be in Git anyway

[package]
name = "websocat-allnodes"
version = "0.1.0"
edition = "2018"

[dependencies]
websocat-api = {path = "../websocat-api", features=["sync_impl"]}

END
;

foreach my $cr (@crates_to_scan) {
    print F "$cr = {path = \"../$cr\"}\n";
}

close F;


open F, ">", "crates/websocat-allnodes/src/lib.rs";

print F <<END
//! This is an auto-generated file based on auto_populate_in_allclasslist annotations, but it is intended to be in Git anyway

/// Get `ClassRegistrar` with all WebSocat's nodes registered
pub fn all_node_classes() -> websocat_api::ClassRegistrar {
    let mut reg = websocat_api::ClassRegistrar::default();
END
;

foreach my $crate (@crates_to_scan) {
    open G, "-|", "cargo rustc -p $crate -- -Zunpretty=expanded --emit=metadata,dep-info";

    $crate =~ y/-/_/;

    my $armed = 0;
    my @modules = ();
    my @modules_indentlevels = ();
    while(<G>) {
        next if m@^\s*$@;
        if (scalar(@modules) > 0) {
            my $indentlen=0;
            if (m@^( *)@) {
                $indentlen=length($1);
            }
            if ($indentlen <= $modules_indentlevels[-1]) {
                #print STDERR "END MOD $modules[-1]\n";
                pop @modules_indentlevels;
                pop @modules;
            }
        }

        if (m@\#\[auto_populate_in_allclasslist\]@) {
            $armed = 1;
        } elsif (m@\#\[auto_populate_macro_in_allclasslist\]@) {
            $armed = 2;
        } elsif (m@\#\[.*\]@) {

        } elsif (m@pub struct ([A-Za-z0-9_]+)@) {
            if ($armed) {
                my $method = "register"; 
                $method="register_macro" if $armed == 2;
                print F "    reg.".$method."::<$crate"."::".(join "", (map {$_ . "::" } @modules)).$1.">();\n";
            }
            $armed = 0;
        } elsif (m@^( *)(?:pub )?mod (\S+)@) {
            #print STDERR "MOD $2\n";
            push @modules, $2;
            my $indentlen=length($1);
            push @modules_indentlevels, $indentlen;
        } else {
            if ($armed) {
                print STDERR "Stray auto_populate_in_allclasslist annotation in $crate";
            }
            $armed = 0;
        }
    }


    close G;
}

print F "    reg\n";
print F "}\n";

close F;

#    reg.register::<websocat_http::HttpClient>();
#    reg.register_macro::<websocat_http::AutoLowlevelHttpClient>();
