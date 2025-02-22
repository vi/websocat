use websocat::test_utils::{test_websocat, test_two_websocats};

macro_rules! t {
    ($n:ident, $x:literal $(,)?) => {
        #[tokio::test]
        async fn $n() {
            test_websocat($x).await;
        }
    };
}

macro_rules! t2 {
    ($n:ident, $x:literal, $y:literal $(,)?) => {
        #[tokio::test]
        async fn $n() {
            test_two_websocats($x, $y).await;
        }
    };
}

t!(dummy, "-b dummy: dummy:");
t!(mock1, "-bU mock_stream_socket:'w ABC' mock_stream_socket:'r ABC'");
t!(mock2, "-b mock_stream_socket:'w QQ|w Q|r A' mock_stream_socket:'r Q|r QQ|w A'");
t!(mock3, "-b mock_stream_socket:'w QQ|w Q' mock_stream_socket:'r Q|r QQ'");
t!(mock4, r#"-bu mock_stream_socket:'r AB\ \|CDE\x00\r\n\t' mock_stream_socket:'w\x41B \x7cCDE\0\r\n\t'"#);

t!(wsll1, r#"-b  chunks:mock_stream_socket:'R ABC'  ws-lowlevel-server:mock_stream_socket:'W \x82\x03ABC' --no-close"#);
t!(wsll2, r#"-b  chunks:mock_stream_socket:'R ABC'  ws-lowlevel-server:mock_stream_socket:'W \x82\x03ABC\x88\x00'"#);
t!(wsll3, r#"-b  chunks:mock_stream_socket:'R ABC'  ws-lowlevel-client:mock_stream_socket:'W \x82\x83\x1d\xfb\x9f\x97\\\xb9\xdc| W \x88\x80\xc5\xca\xbfb' --random-seed 2"#);
t!(wsll4, r#"-b  chunks:mock_stream_socket:'W ABC|R qwerty'  ws-lowlevel-server:mock_stream_socket:'R \x82\x83\x1d\xfb\x9f\x97\\\xb9\xdc|W \x82\x06qwerty\x88\x00'"#);

t!(wsll_pingreply1, r#"-b --no-close --random-seed 3 dummy:  ws-lowlevel-client:mock_stream_socket:'R \x89\x00 | W \x8a\x80\x85\x87T\xbd'"#);
t!(wsll_pingreply2, r#"-b --no-close --random-seed 3 dummy:  ws-lowlevel-client:mock_stream_socket:'R \x89\x03ABC | W \x8a\x83\x85\x87T\xbd\xc4\xc5\x17'"#);
t!(wsll_pingreply3, r#"-b --no-close dummy: ws-lowlevel-server:mock_stream_socket:'R \x89\x83\x85\x87T\xbd\xc4\xc5\x17 | W \x8a\x03ABC'"#);

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
