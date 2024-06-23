# Scenario functions

Those functions are used in Websocat Rhai Scripts (Scenarios):

## connect_tcp

Parameters:

* opts (`Dynamic`)
* continuation (`Fn(StreamSocket)`)

Returns `Task`

## connect_tcp_race

Parameters:

* opts (`Dynamic`)
* addrs (`Vec<SocketAddr>`)
* continuation (`Fn(StreamSocket)`)

Returns `Task`

## copy_bytes

Forward unframed bytes from source to sink

Parameters:

* from (`StreamRead`) - stream source to read from
* to (`StreamWrite`) - stream sink to write to

Returns `Task` - task that finishes when forwarding finishes or exists with an error

## copy_packets

Parameters:

* from (`DatagramRead`)
* to (`DatagramWrite`)

Returns `Task`

## create_stdio

Returns `StreamSocket`

## display_pkts

Returns `DatagramWrite`

## dummy_task

Returns `Task`

## empty_hangup_handle

Returns `Hangup`

## exchange_bytes

Parameters:

* opts (`Dynamic`)
* s1 (`StreamSocket`)
* s2 (`StreamSocket`)

Returns `Task`

## exchange_packets

Parameters:

* opts (`Dynamic`)
* s1 (`DatagramSocket`)
* s2 (`DatagramSocket`)

Returns `Task`

## http1_client

Parameters:

* opts (`Dynamic`)
* inner (`StreamSocket`)

Returns `Http1Client`

## http1_serve

Parameters:

* opts (`Dynamic`)
* inner (`StreamSocket`)
* continuation (`Fn(IncomingRequest, Hangup) -> OutgoingResponse`)

Returns `Task`

## listen_tcp

Parameters:

* opts (`Dynamic`)
* continuation (`Fn(StreamSocket, SocketAddr)`)

Returns `Task`

## lookup_host

Parameters:

* addr (`String`)
* continuation (`Fn(Vec<SocketAddr>)`)

Returns `Task`

## parallel

Parameters:

* tasks (`Vec<Dynamic>`)

Returns `Task`

## put_hangup_part

Parameters:

* h (`Dynamic`)
* x (`Hangup`)

Returns `()`

## put_read_part

Parameters:

* h (`StreamSocket`)
* x (`StreamRead`)

Returns `()`

## put_sink_part

Parameters:

* h (`DatagramSocket`)
* x (`DatagramWrite`)

Returns `()`

## put_source_part

Parameters:

* h (`DatagramSocket`)
* x (`DatagramRead`)

Returns `()`

## put_write_part

Parameters:

* h (`StreamSocket`)
* x (`StreamWrite`)

Returns `()`

## read_chunk_limiter

Parameters:

* x (`StreamRead`)
* limit (`i64`)

Returns `StreamRead`

## read_stream_chunks

Parameters:

* x (`StreamRead`)

Returns `DatagramRead`

## sequential

Parameters:

* tasks (`Vec<Dynamic>`)

Returns `Task`

## sleep_ms

Parameters:

* ms (`i64`)

Returns `Task`

## spawn_task

Parameters:

* task (`Task`)

Does not return anything.

## stream_chunks

Parameters:

* x (`StreamSocket`)

Returns `DatagramSocket`

## take_hangup_part

Parameters:

* h (`Dynamic`)

Returns `Hangup`

## take_read_part

Parameters:

* h (`StreamSocket`)

Returns `StreamRead`

## take_sink_part

Parameters:

* h (`DatagramSocket`)

Returns `DatagramWrite`

## take_source_part

Parameters:

* h (`DatagramSocket`)

Returns `DatagramRead`

## take_write_part

Parameters:

* h (`StreamSocket`)

Returns `StreamWrite`

## tls_client

Parameters:

* opts (`Dynamic`)
* connector (`Arc<tokio_native_tls::TlsConnector>`)
* inner (`StreamSocket`)
* continuation (`Fn(StreamSocket)`)

Returns `Task`

## tls_client_connector

Parameters:

* opts (`Dynamic`)

Returns `Arc<tokio_native_tls::TlsConnector>`

## trivial_pkts

Returns `DatagramRead`

## write_stream_chunks

Parameters:

* x (`StreamWrite`)

Returns `DatagramWrite`

## ws_accept

Parameters:

* opts (`Dynamic`)
* rq (`IncomingRequest`)
* close_handle (`Hangup`)
* continuation (`Fn(StreamSocket)`)

Returns `OutgoingResponse`

## ws_decoder

Parameters:

* opts (`Dynamic`)
* inner (`StreamRead`)

Returns `DatagramRead`

## ws_encoder

Parameters:

* opts (`Dynamic`)
* inner (`StreamWrite`)

Returns `DatagramWrite`

## ws_upgrade

Parameters:

* opts (`Dynamic`)
* client (`Http1Client`)
* continuation (`Fn(StreamSocket)`)

Returns `Task`

## ws_wrap

Parameters:

* opts (`Dynamic`)
* inner (`StreamSocket`)

Returns `DatagramSocket`

