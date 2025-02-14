use websocat::test_utils::test_websocat as websocat;

#[tokio::test]
async fn dummy() {
    websocat("-b dummy: dummy:").await;
}

#[tokio::test]
async fn check_mock_stream_socket1() {
    websocat("-bU mock_stream_socket:'w ABC' mock_stream_socket:'r ABC'").await;
}


#[tokio::test]
async fn check_mock_stream_socket2() {
    websocat("-b mock_stream_socket:'w QQ|w Q|r A' mock_stream_socket:'r Q|r QQ|w A'").await;
}

#[tokio::test]
async fn check_mock_stream_socket3() {
    websocat("-b mock_stream_socket:'w QQ|w Q' mock_stream_socket:'r Q|r QQ'").await;
}
