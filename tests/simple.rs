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
t!(wsll4, r"-b  chunks:mock_stream_socket:'W ABC|R qwerty'  ws-lowlevel-server:mock_stream_socket:'R \x82\x83\x1d\xfb\x9f\x97\\\xb9\xdc|W \x82\x06qwerty\x88\x00'");

t2!(regstr1,
    "-b --oneshot registry-stream-listen: devnull:",
    "-b devnull: registry-stream-connect:",
);
t2!(regstr2,
    "-b --oneshot registry-stream-listen: mock_stream_socket:'R ABC'",
    "-b registry-stream-connect: mock_stream_socket:'W ABC'",
);

