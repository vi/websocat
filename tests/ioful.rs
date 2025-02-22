#![cfg(feature="ioful_tests")]
#![allow(unused)]

use websocat::{t,t2w,t_linux,t2w_linux,t_unix,t2w_unix,t_online,t2w_online,test_utils::{test_websocat,test_two_websocats}};

t2w!(basic, r#"-bs 12000 --oneshot mock_stream_socket:'R qqq\n|W www\n|R eee\n|W tt\n'"#,
           r#"-b ws://127.0.0.1:12000/ mock_stream_socket:'W qqq\n|R www\n|W eee\n|R tt\n'"#);
t_online!(roundtrip, r#"-t ws://ws.vi-server.org/mirror mock_stream_socket:'R 123\n|W 123\n|R 456\n|W 456\n'"#);
#[cfg(feature="ssl")]
t_online!(roundtrip_wss, r#"-t wss://ws.vi-server.org/mirror mock_stream_socket:'R 123\n|W 123\n|R 456\n|W 456\n'"#);

t2w_unix!(unixsock, r#"-t --unlink --oneshot ws-u:unix-listen:/tmp/websocat_test.sock mock_stream_socket:'R qqq\n|W www\n|R eee\n|W tt\n'"#,
          r#"-t ws-c:unix:/tmp/websocat_test.sock  mock_stream_socket:'W qqq\n|R www\n|W eee\n|R tt\n' "#);
t2w_linux!(abstractsock, r#"-t --oneshot ws-u:abstract-listen:websocat_test mock_stream_socket:'R qqq\n|W www\n|R eee\n|W tt\n'"#,
          r#"-t ws-c:abstract:websocat_test  mock_stream_socket:'W qqq\n|R www\n|W eee\n|R tt\n' "#);
t2w_linux!(unix_seqpack, r#"-t --oneshot --unlink seqpacket-listen:/tmp/websocat_test2.sock mock_stream_socket:'R qqq\n|W www\n|R eee\n|W tt\n'"#,
          r#"-t seqpacket:/tmp/websocat_test2.sock  mock_stream_socket:'W qqq\n|R www\n|W eee\n|R tt\n' "#);
t2w_linux!(abstract_seqpack, r#"-t --oneshot seqpacket-abstract-listen:websocat_test2 mock_stream_socket:'R qqq\n|W www\n|R eee\n|W tt\n'"#,
          r#"-t seqpacket-abstract:websocat_test2  mock_stream_socket:'W qqq\n|R www\n|W eee\n|R tt\n' "#);

t_unix!(process, r#"-t exec:cat mock_stream_socket:'R 123\n|W 123\n|R 456\n|W 456\n'"#);
