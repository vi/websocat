use std::ffi::OsStr;

use base64::Engine as _;

use super::{
    scenarioprinter::StrLit,
    types::{Endpoint, ScenarioPrintingEnvironment},
};

pub fn format_osstr(arg: &OsStr) -> String {
    if let Ok(s) = arg.try_into() {
        let s: &str = s;
        return format!("osstr_str({})", StrLit(s));
    }
    #[cfg(any(unix, target_os = "wasi"))]
    {
        #[cfg(unix)]
        use std::os::unix::ffi::OsStrExt;
        #[cfg(all(not(unix), target_os = "wasi"))]
        use std::os::wasi::ffi::OsStrExt;

        let x = base64::prelude::BASE64_STANDARD.encode(arg.as_bytes());
        return format!("osstr_base64_unix_bytes(\"{x}\")");
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let b: Vec<u16> = arg.encode_wide().collect();
        let bb: Vec<u8> =
            Vec::from_iter(b.into_iter().map(|x| u16::to_le_bytes(x))).into_flattened();
        let x = base64::prelude::BASE64_STANDARD.encode(bb);

        return format!("osstr_base64_windows_utf16le(\"{}\")", x);
    }
    #[allow(unreachable_code)]
    {
        let x = base64::prelude::BASE64_STANDARD.encode(arg.as_encoded_bytes());
        format!("osstr_base64_unchecked_encoded_bytes(\"{x}\")")
    }
}

impl Endpoint {
    fn continue_printing_cmd_or_exec(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
        var_cmd: String,
    ) -> anyhow::Result<String> {
        if let Some(ref x) = env.opts.exec_chdir {
            if let Some(s) = x.to_str() {
                env.printer
                    .print_line(&format!("{var_cmd}.chdir({});", StrLit(s)));
            } else {
                env.printer.print_line(&format!(
                    "{var_cmd}.chdir_osstr({});",
                    format_osstr(x.as_os_str())
                ));
            }
        }

        if let Some(ref x) = env.opts.exec_arg0 {
            if let Some(s) = x.to_str() {
                env.printer
                    .print_line(&format!("{var_cmd}.arg0({});", StrLit(s)));
            } else {
                env.printer.print_line(&format!(
                    "{var_cmd}.arg0_osstr({});",
                    format_osstr(x.as_os_str())
                ));
            }
        }

        if let Some(x) = env.opts.exec_uid {
            env.printer.print_line(&format!("{var_cmd}.uid({x});"));
        }
        if let Some(x) = env.opts.exec_gid {
            env.printer.print_line(&format!("{var_cmd}.gid({x});"));
        }
        if let Some(x) = env.opts.exec_windows_creation_flags {
            env.printer
                .print_line(&format!("{var_cmd}.windows_creation_flags({x});"));
        }

        let var_chld = env.vars.getnewvarname("chld");
        let var_s = env.vars.getnewvarname("pstdio");

        if env.opts.exec_dup2.is_none() {
            env.printer
                .print_line(&format!("{var_cmd}.configure_fds(2, 2, 1);"));
            env.printer
                .print_line(&format!("let {var_chld} = {var_cmd}.execute();"));
            env.printer
                .print_line(&format!("let {var_s} = {var_chld}.socket();"));

            if env.opts.exec_monitor_exits {
                env.printer
                    .print_line(&format!("put_hangup_part({var_s}, {var_chld}.wait());"));
            }
            Ok(var_s)
        } else {
            env.printer
                .print_line(&format!("{var_cmd}.configure_fds(1, 1, 1);"));

            Ok(var_cmd)
        }
    }

    pub(super) fn begin_print_exec(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::Exec(s) => {
                let var_cmd = env.vars.getnewvarname("cmd");
                if let Ok(s) = s.as_os_str().try_into() {
                    let s: &str = s;
                    env.printer
                        .print_line(&format!("let {var_cmd} = subprocess_new({});", StrLit(s)));
                } else {
                    env.printer.print_line(&format!(
                        "let {var_cmd} = subprocess_new_osstr({});",
                        format_osstr(s)
                    ));
                }

                for arg in &env.opts.exec_args {
                    if let Some(s) = arg.to_str() {
                        env.printer
                            .print_line(&format!("{var_cmd}.arg({});", StrLit(s)));
                    } else {
                        env.printer
                            .print_line(&format!("{var_cmd}.arg_osstr({});", format_osstr(arg)));
                    }
                }

                self.continue_printing_cmd_or_exec(env, var_cmd)
            }
            Endpoint::Cmd(s) => {
                let var_cmd = env.vars.getnewvarname("cmd");
                if cfg!(windows) {
                    env.printer
                        .print_line(&format!("let {var_cmd} = subprocess_new(\"cmd\");"));
                    env.printer.print_line(&format!("{var_cmd}.arg(\"/C\");",));
                    env.printer
                        .print_line(&format!("{var_cmd}.raw_windows_arg({});", format_osstr(s)));
                } else {
                    env.printer
                        .print_line(&format!("let {var_cmd} = subprocess_new(\"sh\");"));
                    env.printer.print_line(&format!("{var_cmd}.arg(\"-c\");",));
                    if let Ok(s) = s.as_os_str().try_into() {
                        let s: &str = s;
                        env.printer
                            .print_line(&format!("{var_cmd}.arg({});", StrLit(s)));
                    } else {
                        env.printer
                            .print_line(&format!("{var_cmd}.arg_osstr({});", format_osstr(s)));
                    }
                }

                self.continue_printing_cmd_or_exec(env, var_cmd)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_exec(&self, _env: &mut ScenarioPrintingEnvironment<'_>) {
        match self {
            Endpoint::Exec(_) => {}
            Endpoint::Cmd(_) => {}
            _ => panic!(),
        }
    }
}
