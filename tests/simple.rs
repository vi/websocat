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

