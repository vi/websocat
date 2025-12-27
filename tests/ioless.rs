#![cfg_attr(rustfmt, rustfmt::skip)]
use websocat::{t,t2,t3w_p,t_p};

t!(dummy, "-b dummy: dummy:");
t!(mock1, "-bU mock_stream_socket:'w ABC' mock_stream_socket:'r ABC'");
t!(mock2, "-b mock_stream_socket:'w QQ|w Q|r A' mock_stream_socket:'r Q|r QQ|w A'");
t!(mock3, "-b mock_stream_socket:'w QQ|w Q' mock_stream_socket:'r Q|r QQ'");
t!(mock4, r#"-bu mock_stream_socket:'r AB\ \|CDE\x00\r\n\t' mock_stream_socket:'w\x41B \x7cCDE\0\r\n\t'"#);

t!(wsll1, r#"-b  chunks:mock_stream_socket:'R ABC'  ws-lowlevel-server:mock_stream_socket:'W \x82\x03ABC' --no-close"#);
t!(wsll1b, r#"-b  --inhibit-pongs=0 chunks:mock_stream_socket:'R ABC'  ws-lowlevel-server:mock_stream_socket:'W \x82\x03ABC' --no-close"#);
t!(wsll2, r#"-b  chunks:mock_stream_socket:'R ABC'  ws-lowlevel-server:mock_stream_socket:'W \x82\x03ABC\x88\x00'"#);
t!(wsll2b, r#"-b --inhibit-pongs=0  chunks:mock_stream_socket:'R ABC'  ws-lowlevel-server:mock_stream_socket:'W \x82\x03ABC\x88\x00'"#);
t!(wsll3, r#"-b  chunks:mock_stream_socket:'R ABC'  ws-lowlevel-client:mock_stream_socket:'W \x82\x83\x1d\xfb\x9f\x97\\\xb9\xdc| W \x88\x80\xc5\xca\xbfb' --random-seed 2"#);
t!(wsll4, r#"-b  chunks:mock_stream_socket:'W ABC|R qwerty'  ws-lowlevel-server:mock_stream_socket:'R \x82\x83\x1d\xfb\x9f\x97\\\xb9\xdc|W \x82\x06qwerty\x88\x00'"#);
t!(wsll5, r#"-b  chunks:mock_stream_socket:'W ABC|R qwerty' --inhibit-pongs=0  ws-lowlevel-server:mock_stream_socket:'R \x82\x83\x1d\xfb\x9f\x97\\\xb9\xdc|W \x82\x06qwerty\x88\x00'"#);

t!(wsll_pingreply1, r#"-b --no-close --random-seed 3 dummy:  ws-lowlevel-client:mock_stream_socket:'R \x89\x00| W \x8a\x80\x85\x87T\xbd'"#);
t!(wsll_pingreply2, r#"-b --no-close --random-seed 3 dummy:  ws-lowlevel-client:mock_stream_socket:'R \x89\x03ABC| W \x8a\x83\x85\x87T\xbd\xc4\xc5\x17'"#);
t!(wsll_pingreply3, r#"-b --no-close dummy: ws-lowlevel-server:mock_stream_socket:'R \x89\x83\x85\x87T\xbd\xc4\xc5\x17| W \x8a\x03ABC'"#);
t!(wsll_pingreply4, r#"-b --no-close --inhibit-pongs=0 --random-seed 3 dummy:  ws-lowlevel-client:mock_stream_socket:'R \x89\x00'"#);
t!(wsll_pingreply5, r#"-b --no-close --inhibit-pongs=1 --random-seed 3 dummy:  ws-lowlevel-client:mock_stream_socket:'R \x89\x00| W \x8a\x80\x85\x87T\xbd|R \x89\x00|R \x89\x00'"#);
t!(wsll_pingreply6, r#"-b --no-close --inhibit-pongs=2 --random-seed 3 dummy:  ws-lowlevel-client:mock_stream_socket:'R \x89\x00| W \x8a\x80\x85\x87T\xbd|R \x89\x00|R \x89\x00| W \x8a\x80\x6c\xe4\x13\x63'"#);

t!(wsll_pingreply_payload1, r#"-b --no-close --random-seed 3 dummy:  ws-lowlevel-client:mss:'R \x89\x04ABCD| W \x8a\x84\x85\x87T\xbd| W \xC4\xC5\x17\xF9'"#);
t!(wsll_pingreply_payload2, r#"-b --no-close --random-seed 3 dummy:  ws-lowlevel-client:read_chunk_limiter:mss:'R \x89\x04ABCD| W \x8a\x84\x85\x87T\xbd| W \xC4\xC5\x17\xF9'"#);
t!(wsll_pingreply_payload3, r#"-b --inhibit-pongs=1 --no-close --random-seed 3 dummy:  ws-lowlevel-client:read_chunk_limiter:mss:'R \x89\x04ABCD| X | W \x8a\x84\x85\x87T\xbd| W \xC4\xC5\x17\xF9'"#);

t2!(regstr1,
    "-b --oneshot registry-stream-listen: devnull:",
    "-b devnull: registry-stream-connect:",
);
t2!(regstr2,
    "-b --oneshot registry-stream-listen: mock_stream_socket:'R ABC'",
    "-b registry-stream-connect: mock_stream_socket:'W ABC'",
);

t2!(wsupg1,
    "-b --oneshot ws-accept:registry-stream-listen: dummy:",
    "-b dummy: ws-request:registry-stream-connect:",
);

t2!(ws_roundtrip1,
    "-b --oneshot ws-upgrade:registry-stream-listen: chunks:mock_stream_socket:'W ABC | R 0123 | W DEF'",
    "-b chunks:mock_stream_socket:'R ABC | W 0123 | R DEF' ws-connect:registry-stream-connect:",
);

t!(lpr1, r#"-ub --lengthprefixed-skip-read-direction mock_stream_socket:'R ABC| R 111|R a22aa\n' lengthprefixed:mock_stream_socket:'W \0\0\0\x03ABC|W \0\0\0\x03111|W \0\0\0\x06a22aa\n'"#);
t!(lpr2, r#"-ub --lengthprefixed-skip-read-direction --lengthprefixed-nbytes 3 --lengthprefixed-little-endian mock_stream_socket:'R ABC| R 111|R a22aa\n' lengthprefixed:mock_stream_socket:'W \x03\0\0ABC|W \x03\0\0111|W \x06\0\0a22aa\n'"#);
t!(wslpr1, r#"-ubn  --lengthprefixed-skip-read-direction  ws-lowlevel-client:mock_stream_socket:'R \x82\x00' lengthprefixed:mock_stream_socket:'W \0\0\0\0' "#);
t!(wslpr2, r#"-ubn  --lengthprefixed-skip-read-direction  ws-lowlevel-client:mock_stream_socket:'R \x82\x03ABC' lengthprefixed:mock_stream_socket:'W \0\0\0\x03ABC' "#);
t!(wslpr3, r#"-ubn  --lengthprefixed-skip-read-direction --lengthprefixed-tag-text  ws-lowlevel-client:mock_stream_socket:'R \x82\x03ABC' lengthprefixed:mock_stream_socket:'W \0\0\0\x03ABC' "#);
t!(wslpr4, r#"-ubn  --lengthprefixed-skip-read-direction --lengthprefixed-tag-text  ws-lowlevel-client:mock_stream_socket:'R \x81\x03ABC' lengthprefixed:mock_stream_socket:'W \x80\0\0\x03ABC' "#);
t!(wslpr5, r#"-ubn  --lengthprefixed-skip-read-direction --lengthprefixed-tag-text  ws-lowlevel-client:mock_stream_socket:'R \x81\x03ABC| R \x82\x02QQ| R\x88\0' lengthprefixed:mock_stream_socket:'W \x80\0\0\x03ABC| W \0\0\0\x02QQ' "#);

t!(wslprc1, r#"-ubn --lengthprefixed-include-control --lengthprefixed-skip-read-direction 
             ws-lowlevel-client:mock_stream_socket:'R \x88\x03ABC'
             lengthprefixed:mock_stream_socket:'W \x80\x00\x00\x04\x08ABC'
"#);
t!(wslprc2, r#"-ubn --random-seed 3 --lengthprefixed-include-control --lengthprefixed-skip-read-direction --inhibit-pongs=1
             ws-lowlevel-client:mock_stream_socket:'R \x89\x00|W \x8a\x80\x85\x87T\xbd|R \x89\x00'
             lengthprefixed:mock_stream_socket:'W \x80\x00\x00\x01\x09| W \x80\0\0\x01\x08'
"#);
t!(wslprc3, r#"-ubn --lengthprefixed-include-control --lengthprefixed-skip-read-direction 
             ws-lowlevel-client:mock_stream_socket:'R \x8A\x03ABC'
             lengthprefixed:mock_stream_socket:'W \x80\x00\x00\x04\x0AABC| W \x80\0\0\x01\x08'
"#);
t!(wslprc4, r#"-ubn --lengthprefixed-include-control --lengthprefixed-continuations  --lengthprefixed-skip-read-direction 
             ws-lowlevel-client:mock_stream_socket:'R \x88\x03ABC'
             lengthprefixed:mock_stream_socket:'W \x40\x00\x00\x04\x08ABC'
"#);
t!(wslprc5, r#"-ubn --random-seed 3 --lengthprefixed-include-control --lengthprefixed-continuations  --lengthprefixed-skip-read-direction --inhibit-pongs=1
             ws-lowlevel-client:mock_stream_socket:'R \x89\x00|W \x8a\x80\x85\x87T\xbd|R \x89\x00'
             lengthprefixed:mock_stream_socket:'W \x40\x00\x00\x01\x09| W \x40\0\0\x01\x08'
"#);
t!(wslprc6, r#"-ubn --lengthprefixed-include-control --lengthprefixed-continuations  --lengthprefixed-skip-read-direction 
             ws-lowlevel-client:mock_stream_socket:'R \x8A\x03ABC'
             lengthprefixed:mock_stream_socket:'W \x40\x00\x00\x04\x0AABC| W \x40\0\0\x01\x08'
"#);
t!(wslprcnt1, r#"-ubn --lengthprefixed-continuations  --lengthprefixed-skip-read-direction 
             ws-lowlevel-client:mock_stream_socket:'R \x01\x03ABC| R \x80\x03DEF'
             lengthprefixed:mock_stream_socket:'W \x80\x00\x00\x03ABC| W \0\0\0\x03DEF'
"#);
t!(wslprcnt2, r#"-ubn --lengthprefixed-skip-read-direction 
             ws-lowlevel-client:mock_stream_socket:'R \x01\x03ABC| R \x80\x03DEF'
             lengthprefixed:mock_stream_socket:'W \0\0\0\x06ABCDEF'
"#);
t!(wslprcnt3, r#"-ubn --lengthprefixed-include-control --lengthprefixed-continuations --inhibit-pongs=0  --lengthprefixed-skip-read-direction
            ws-lowlevel-client:read_chunk_limiter:mock_stream_socket:'R \x89\x03ABC'
            lengthprefixed:mock_stream_socket:'W \xc0\x00\x00\x02\x09A|W \xc0\x00\x00\x02\x09B|W \x40\x00\x00\x02\x09C|W \x40\0\0\x01\x08'
"#);
t!(wslprcnt4, r#"-ubn --lengthprefixed-include-control --inhibit-pongs=0  --lengthprefixed-skip-read-direction
            ws-lowlevel-client:read_chunk_limiter:mock_stream_socket:'R \x89\x03ABC'
            lengthprefixed:mock_stream_socket:'W \x80\x00\x00\x04\x09ABC|W \x80\0\0\x01\x08'
"#);
t!(wllpr_short1, r#"-ubn --lengthprefixed-include-control --lengthprefixed-continuations --inhibit-pongs=0  --lengthprefixed-skip-read-direction
            ws-lowlevel-client:read_chunk_limiter:mock_stream_socket:'R \x89\x03ABC'
            lengthprefixed:write_chunk_limiter:mock_stream_socket:'W \xc0\x00\x00\x02\x09A|W \xc0\x00\x00\x02\x09B|W \x40\x00\x00\x02\x09C|W \x40\0\0\x01\x08'
"#);
t!(wllpr_short2, r#"-ubn --lengthprefixed-include-control --inhibit-pongs=0  --lengthprefixed-skip-read-direction
            ws-lowlevel-client:read_chunk_limiter:mock_stream_socket:'R \x89\x03ABC'
            lengthprefixed:write_chunk_limiter:mock_stream_socket:'W \x80\x00\x00\x04\x09ABC|W \x80\0\0\x01\x08'
"#);

t!(lprr1, r#"-ub --lengthprefixed-include-control
             lengthprefixed:read_chunk_limiter:mock_stream_socket:'R \x80\x00\x00\x04\x09GGG|R \0\0\0\x0555555|W \x80\0\0\x01\x08'
             lengthprefixed:mock_stream_socket:'W \x80\x00\x00\x04\tGGG|W \x00\x00\x00\x0555555|W \x80\x00\x00\x01\x08'  "#);
t!(lprr2, r#" -ub --lengthprefixed-include-control --read-buffer-limit=2  --lengthprefixed-nbytes=1 --lengthprefixed-continuations 
             lengthprefixed:read_chunk_limiter:mock_stream_socket:'R \x44\x09GGG|R \x0555555|W \x41\x08'
             lengthprefixed:mock_stream_socket:'W \xc3\x09GG\x42\x09G\x8255\x8255\x015\x41\x08'  "#);
t!(lprr3, r#" -ub --lengthprefixed-tag-text --read-buffer-limit=1  --lengthprefixed-nbytes=1
             lengthprefixed:read_chunk_limiter:mock_stream_socket:'R \x83GGG|R \x0555555'
             lengthprefixed:mock_stream_socket:'W \x83GGG|W \x0555555'  "#);

t!(line1, r#"-ut  --lengthprefixed-skip-read-direction mock_stream_socket:'R abcdef\n' lengthprefixed:mock_stream_socket:'W \0\0\0\x06abcdef' "#);
t!(line2, r#"-ut  --lengthprefixed-skip-read-direction mock_stream_socket:'R ab|R cde|R f\n' lengthprefixed:mock_stream_socket:'W \0\0\0\x06abcdef' "#);
t!(line3, r#"-ut  --lengthprefixed-skip-read-direction mock_stream_socket:'R abcdef|R \n' lengthprefixed:mock_stream_socket:'W \0\0\0\x06abcdef' "#);
t!(line4, r#"-ut  --lengthprefixed-skip-read-direction 
                                                mock_stream_socket:'R ab\ncde\nf|R \n|R QWE\n| R RTY\n\n'
                                                lengthprefixed:mock_stream_socket:'W \0\0\0\x02ab|W \0\0\0\x03cde|W \0\0\0\x01f|W \0\0\0\x03QWE| W \0\0\0\x03RTY| W \0\0\0\0' "#);
t!(line5, r#"-ut  --lengthprefixed-skip-read-direction  --separator-n 2 
                                                mock_stream_socket:'R ab\ncde\nf|R \n|R QWE\n| R RTY\n\n'
                                                lengthprefixed:mock_stream_socket:'W \x00\x00\x00\x10ab\ncde\nf\nQWE\nRTY'"#);
t!(line6, r#"-ut  --lengthprefixed-skip-read-direction  --separator-n 2
                                                mock_stream_socket:'R ab\n\ncde\n|R \n|R QWE\n| R \nRTY\n\n| R \n\n\n\n| R \n\n+\n\n'
                                                lengthprefixed:mock_stream_socket:'W \0\0\0\x02ab|W \0\0\0\x03cde|W \0\0\0\x03QWE| W \0\0\0\x03RTY| W \0\0\0\0| W \0\0\0\0| W \0\0\0\0| W \0\0\0\x01+'"#);

t!(linew1, r#"-ut 
                    chunks:mock_stream_socket:'R abc|R def\n|R \nQWE\n\n'
                    mock_stream_socket:'W abc\n|W def \n|W QWE  \n'"#);
t!(linew2, r#"-ut --separator=0
                    chunks:mock_stream_socket:'R abc|R def\n|R \nQWE\n\n'
                    mock_stream_socket:'W abc\0|W def\n\0|W \nQWE\n\n\0'"#);
t!(linew3, r#"-ut --separator-inhibit-substitution
                    chunks:mock_stream_socket:'R abc|R def\n|R \nQWE\n\n'
                    mock_stream_socket:'W abc\n|W def\n\n|W \nQWE\n\n\n'"#);
t!(linew4, r#"-ut --separator-n=2
                    chunks:mock_stream_socket:'R abc|R def\n|R \nQWE\n\n| R 456\n678\n'
                    mock_stream_socket:'W abc\n\n|W def\n\n|W QWE\n \n\n|W 456\n678\n\n'"#);
t!(linew5, r#"-ut 
                    chunks:mock_stream_socket:'R abc|R def\n|R \nQWE\n\n'
                    lines:write_chunk_limiter:mock_stream_socket:'W abc\n|W def \n|W QWE  \n'"#);
t!(linew6, r#"-ut --separator-n=2
                    chunks:mock_stream_socket:'R abc|R def\n|R \nQWE\n\n| R 456\n678\n'
                    lines:write_chunk_limiter:mock_stream_socket:'W abc\n\n|W def\n\n|W QWE\n \n\n|W 456\n678\n\n'"#);


t!(reuser1, r#"-b chunks:mock_stream_socket:'R abc|R def|W 123|W 456|R qwerty' reuse-raw:chunks:mock_stream_socket:'W abc|W def|R 123|R 456|W qwerty'"#);
t!(reuser2, r#"-b chunks:mock_stream_socket:'W abc|R def|W 123|W 456|R qwerty' reuse-raw:chunks:mock_stream_socket:'R abc|W def|R 123|R 456|W qwerty'"#);
t2!(reuser3,r#"-b --oneshot chunks:registry-stream-listen: reuse-raw:chunks:mock_stream_socket:'R abc|W def|R 123|R 456|W qwerty'"#,
            r#"-b chunks:registry-stream-connect: chunks:mock_stream_socket:'W abc|R def|W 123|W 456|R qwerty'"#);
t2!(reuser4,r#"-b --oneshot chunks:registry-stream-listen: reuse-raw:chunks:mock_stream_socket:'R abc|W def|R 123|R 456|W qwerty'"#,
            r#"-b chunks:registry-stream-connect: chunks:mock_stream_socket:'W abc|R def|W 123|W 456|R qwerty'"#);
t3w_p!(reuser5,r#"-b --global-timeout-ms=5000 chunks:registry-stream-listen: reuse-raw:chunks:mock_stream_socket:'W 123|W QWE|W 456'"#,
            r#"-b chunks:registry-stream-connect: chunks:mock_stream_socket:'R 123|T333|R 456'"#,
            r#"-b chunks:registry-stream-connect: chunks:mock_stream_socket:'R QWE'"#);
t3w_p!(reuser6,r#"-b --global-timeout-ms=5000 -E chunks:registry-stream-listen: reuse-raw:chunks:mock_stream_socket:'T6|R 123'"#,
            r#"-b --global-timeout-ms=2000 chunks:registry-stream-connect: chunks:mock_stream_socket:'W 123'"#,
            r#"-b --global-timeout-ms=50 chunks:registry-stream-connect: chunks:mock_stream_socket:''"#);
t3w_p!(reuser7,r#"-b --global-timeout-ms=5000 -E chunks:registry-stream-listen: reuse-raw:chunks:mock_stream_socket:'T6|R 123'"#,
            r#"-b --global-timeout-ms=50 chunks:registry-stream-connect: chunks:mock_stream_socket:''"#,
            r#"-b --global-timeout-ms=2000 chunks:registry-stream-connect: chunks:mock_stream_socket:'W 123'"#);
t3w_p!(reuser_torn1,r#"-b --global-timeout-ms=5000 --lengthprefixed-nbytes=1 --lengthprefixed-continuations -E 
                      chunks:registry-stream-listen: reuse-raw:lengthprefixed:mock_stream_socket:'R \x03AAA|T900|R \x03BBB|R \x03CCC'"#,
            r#"-b --global-timeout-ms=50 chunks:registry-stream-connect: chunks:mock_stream_socket:'W AAA'"#,
            r#"-b --global-timeout-ms=2000 chunks:registry-stream-connect: chunks:mock_stream_socket:'W BBB|W CCC'"#);
t3w_p!(reuser_torn2,r#"-b --global-timeout-ms=5000 --lengthprefixed-nbytes=1 --lengthprefixed-continuations -E 
                      chunks:registry-stream-listen: reuse-raw:lengthprefixed:mock_stream_socket:'R \x83AAA|T900|R \x03BBB|R \x03CCC'"#,
            r#"-b --global-timeout-ms=50 chunks:registry-stream-connect: chunks:mock_stream_socket:'W AAA'"#,
            r#"-b --global-timeout-ms=2000 chunks:registry-stream-connect: chunks:mock_stream_socket:'W CCC'"#);
t3w_p!(reuser_torn3,r#"-b --global-timeout-ms=5000 --lengthprefixed-nbytes=1 --lengthprefixed-continuations -u
                      lengthprefixed:registry-stream-listen: reuse-raw:lengthprefixed:mock_stream_socket:'W \x03AAA|W \x03BBB|W \x02CC|W \x02DD|W \x01E|W \x01F'"#,
            r#"-b --global-timeout-ms=2000 -U registry-stream-connect: mock_stream_socket:'R \x03AAA|T333|R \x02CC|T333|R \x01E'"#,
            r#"-b --global-timeout-ms=2000 -U registry-stream-connect: mock_stream_socket:'R \x03BBB|T333|R \x02DD|T333|R \x01F'"#);
t3w_p!(reuser_torn4,r#"-b --global-timeout-ms=5000 --lengthprefixed-nbytes=1 --lengthprefixed-continuations -u
                      lengthprefixed:registry-stream-listen: reuse-raw:lengthprefixed:mock_stream_socket:'W \x83AAA|W \x02CC|W \x83BBB|W \x02DD|W \x01E|W \x01F'"#,
            r#"-b --global-timeout-ms=2000 -U registry-stream-connect: mock_stream_socket:'R \x83AAA|T333|R \x02CC|T333|R \x01E'"#,
            r#"-b --global-timeout-ms=2000 -U registry-stream-connect: mock_stream_socket:'R \x83BBB|T333|R \x02DD|T333|R \x01F'"#);
t3w_p!(reuser_torn5,r#"-b --global-timeout-ms=5000 --lengthprefixed-nbytes=1 --lengthprefixed-continuations -u
                      lengthprefixed:registry-stream-listen: reuse-raw:lengthprefixed:mock_stream_socket:'W \x83AAA|W \x82CC|W \x01E|W \x83BBB|W \x02DD|W \x01F'"#,
            r#"-b --global-timeout-ms=2000 -U registry-stream-connect: mock_stream_socket:'R \x83AAA|T333|R \x82CC|T333|R \x01E'"#,
            r#"-b --global-timeout-ms=2000 -U registry-stream-connect: mock_stream_socket:'R \x83BBB|T333|R \x02DD|T333|R \x01F'"#);
// t3w_p!(reuser_torn6,r#"-b --global-timeout-ms=5000 --lengthprefixed-nbytes=2 --lengthprefixed-include-control --lengthprefixed-continuations -E
//                       lengthprefixed:registry-stream-listen: reuse-raw:lengthprefixed:mock_stream_socket:'W \x80\x03AAA|W \x80\x02CC'"#,
//             r#"-b --global-timeout-ms=2000 registry-stream-connect: mock_stream_socket:'R \x80\x03AAA|T333|R \x80\x02CC'"#,
//             r#"-b --global-timeout-ms=3000 registry-stream-connect: mock_stream_socket:'R \x80\x03BBB|T333|R \x00\x02DD|T333|R \x00\x01F'"#);
// t3w_p!(reuser_torn6,r#"-b --global-timeout-ms=5000 --lengthprefixed-nbytes=2 --lengthprefixed-include-control --lengthprefixed-continuations
//                        lengthprefixed:registry-stream-listen: reuse-raw:lengthprefixed:mock_stream_socket:'N sink|W \x80\x03AAA|W \x80\x02CC|W \x40\x62\x08Partially written message to Websocat\x27s `reuse-raw:` prevents further messages in this connection|W \x80\x03BBB'"#,
//              r#"-b --global-timeout-ms=2000 registry-stream-connect: mock_stream_socket:'N first|R \x80\x03AAA|T333|R \x80\x02CC|ER'"#,
//              r#"-b --global-timeout-ms=3000 registry-stream-connect: mock_stream_socket:'N second|R \x80\x03BBB|T333|R \x00\x02DD|T333|R \x00\x01F'"#);
/*t3w_p!(reuser_torn7,r#"-b --global-timeout-ms=5000 --lengthprefixed-nbytes=1 --lengthprefixed-continuations -u --reuser-tolerate-torn-msgs
                      lengthprefixed:registry-stream-listen: reuse-raw:lengthprefixed:mock_stream_socket:'W \x83AAA|W \x82CC|W \x00|W \x83BBB\x02DD\x01F'"#,
            r#"-b --global-timeout-ms=2000 -U registry-stream-connect: mock_stream_socket:'R \x83AAA|T333|R \x82CC'"#,
            r#"-b --global-timeout-ms=2000 -U registry-stream-connect: mock_stream_socket:'R \x83BBB|T333|R \x02DD|T333|R \x01F'"#);*/

t!(writesplt1, r#"-b mock_stream_socket:'R ABC|R WWW|W SSS'  write-splitoff:mock_stream_socket:'R SSS' --write-splitoff=mock_stream_socket:'W ABC|W WWW'  "#);
t!(writesplt2, r#"-b mock_stream_socket:'R ABC|R WWW|W SSS'  write-splitoff:chunks:mock_stream_socket:'R SSS' --write-splitoff=mock_stream_socket:'W ABC|W WWW'  "#);
t!(writesplt3, r#"-b mock_stream_socket:'R ABC|R WWW|W SSS'  write-splitoff:mock_stream_socket:'R SSS' --write-splitoff=chunks:mock_stream_socket:'W ABC|W WWW'  "#);
t!(writesplt4, r#"-b chunks:mock_stream_socket:'R ABC|R WWW|W SSS'  write-splitoff:mock_stream_socket:'R SSS' --write-splitoff=mock_stream_socket:'W ABC|W WWW'  "#);

t!(composed1, r#"--compose -bu mock_stream_socket:'R ABC' registry-stream-connect:qqq '&' 
                           -bu --oneshot registry-stream-listen:qqq mock_stream_socket:'W ABC'"#);
t_p!(composed2, r#"--compose '('
        -bu chunks:mock_stream_socket:'R ABC' registry-stream-connect:qqq 
     ';' 
        -bu chunks:mock_stream_socket:'R 0123' registry-stream-connect:qqq 
     ')'
      '&' 
     -bu registry-stream-listen:qqq reuse-raw:chunks:mock_stream_socket:'W ABC|W 0123' --global-timeout-ms=500"#);

t!(filter1, r#"-bu mock_stream_socket:'R ABC' mock_stream_socket:'W DEF' --filter=mock_stream_socket:'W ABC|R DEF' "#);
t!(filter2, r#"-bu mock_stream_socket:'R ABC' mock_stream_socket:'W ABC' --filter-reverse=mock_stream_socket:'' "#);
t!(filter3, r#"-bU mock_stream_socket:'W DEF' mock_stream_socket:'R ABC' --filter-reverse=mock_stream_socket:'W ABC|R DEF' "#);
t!(filter4, r#"-bU mock_stream_socket:'W GHI' mock_stream_socket:'R ABC' --filter-reverse=mock_stream_socket:'W ABC|R DEF' --filter-reverse=mock_stream_socket:'W DEF|R GHI' "#);
t!(filter5, r#"-b mock_stream_socket:'R X0|W X3' mock_stream_socket:'W X1|R X2' --filter=mock_stream_socket:'W X0|R X1' --filter-reverse=mock_stream_socket:'W X2|R X3' "#);
t!(filter6, r#"-bE mock_stream_socket:'R X0|W X3' mock_stream_socket:'W X1|R X2' --filter=mock_stream_socket:'W X0|R X1' --filter-reverse=mock_stream_socket:'W X2|R X3' "#);
t!(filter7, r#"-b chunks:mock_stream_socket:'R X0|W X4' mock_stream_socket:'W X1|R X2' --filter=mock_stream_socket:'W X0|R X1' --filter-reverse=mock_stream_socket:'W X2|R X3' --filter-reverse=mock_stream_socket:'W X3|R X4' "#);

t!(defragment1, r#"-bu --lengthprefixed-nbytes=1 --lengthprefixed-continuations lengthprefixed:mss:'R \x83ABC|R \x02DE' defragment:lengthprefixed:mss:'W \x05ABCDE'"#);

t!(tee1, r#"-bu chunks:mss:'R 1234' tee:chunks:mss:'W 1234' "#);
t!(tee2, r#"-bu chunks:mss:'R 1234|R 3456|R QQQ' tee:chunks:mss:'W 1234|W 3456|W QQQ' "#);
t!(tee3, r#"-bu chunks:mss:'R 1234' tee:chunks:mss:'W 1234' --tee=chunks:mss:'W 1234'"#);
t!(tee4, r#"-bu chunks:mss:'R 1234|R 3456|R QQQ' tee:chunks:mss:'W 1234|W 3456|W QQQ' --tee=chunks:mss:'W 1234|W 3456|W QQQ'"#);
t!(tee5, r#"-bu chunks:mss:'R 1234|R 3456|R QQQ' tee:chunks:mss:'W 1234|EW' --tee=chunks:mss:'W 1234|W 3456|W QQQ'"#);
t!(tee6, r#"-bu chunks:mss:'R 1234|R 3456' tee:chunks:mss:'W 1234|EW' --tee=chunks:mss:'W 1234' --tee-propagate-failures"#);
t_p!(tee7, r#"-bu chunks:mss:'R 1|R 2|R 3' tee:chunks:mss:'W 1|T100|W 2|T900|W 3' --tee=chunks:mss:'W 1|W 2|T333|W 3'"#);

t!(teee10, r#"-bU chunks:mss:'W 1|W 2|W 3' tee:chunks:mss:'D|R 1|R 2|R 3'"#);
t_p!(teee11, r#"-bU chunks:mss:'W 1|W 2|W 3' tee:chunks:mss:'R 1|T30|R 3' --tee=chunks:mss:'D|T10|R 2'"#);
t_p!(teee12, r#"-bU chunks:mss:'W 1|W 3|W 2' tee:chunks:mss:'R 1|T10|R 3' --tee=chunks:mss:'D|T30|R 2'"#);
t_p!(teee12b, r#"-bU chunks:mss:'W 1|W 3|W 2' tee:chunks:mss:'R 1|T10|R 3' --tee=chunks:mss:'T30|R 2' --unidirectional-late-drop"#);
t_p!(teee13, r#" --lengthprefixed-nbytes=1 --lengthprefixed-continuations -bU defragment:chunks:mss:'W 11|W 22|W 33' 
   tee:lengthprefixed:mss:'R \x811||R \x011|R \x812|R \x012|R \x813|R \x013' "#);
t_p!(teee14, r#" --lengthprefixed-nbytes=1 --lengthprefixed-continuations -bU defragment:chunks:mss:'W 11|W 22|W 33' 
   tee:lengthprefixed:mss:'R \x811||R \x011|T100|R \x813|R \x013' 
   --tee=lengthprefixed:mss:'T2|R \x812|R \x012'"#);
t_p!(teee15, r#" --lengthprefixed-nbytes=1 --lengthprefixed-continuations -bU defragment:chunks:mss:'W 11|W 22|W 33' 
   tee:lengthprefixed:mss:'R \x811||T30|R \x011|T30|R \x813|T30|R \x013' 
   --tee=lengthprefixed:mss:'T1|R \x812|T100|R \x012'"#);
t_p!(teee16, r#" --lengthprefixed-nbytes=1 --lengthprefixed-continuations -bU defragment:chunks:mss:'W 11' 
   tee:lengthprefixed:mss:'R \x811||T30|R \x011|T30' 
   --tee=lengthprefixed:mss:'T1|R \x812'"#);
t_p!(teee17, r#" --lengthprefixed-nbytes=1 --lengthprefixed-continuations -bU defragment:chunks:mss:'W 11|W 2|W 33' 
   tee:lengthprefixed:mss:'R \x811||T30|R \x011|T30|R \x813|T30|R \x013' 
   --tee=lengthprefixed:mss:'T1|R \x812'
   --tee-tolerate-torn-msgs"#);
