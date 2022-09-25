#ifndef WEBSOCAT_TRANSFORM_H
#define WEBSOCAT_TRANSFORM_H

#include <stddef.h>

#ifndef DLLEXPORT
#define DLLEXPORT
#endif

#ifndef DLLIMPORT
#define DLLIMPORT
#endif

/// This function is called by Websocat to let you transform your data arbitrarily.
///
/// `buffer` - buffer both for reading input data and for writing transformed data
/// `data_length` - number of filled in bytes for transforming in the buffer
/// `buffer_capacity` - number of bytes that may be written to the buffer
/// `connection_number` - sequence number of a Websocat session being transformed. If multiple transforms are in used, expect gaps in sequence numbers.
/// `packet_number` - sequence number of `websocat_transform` invocation within one connection, starting from 1.
///
///  Return value is number of bytes in the buffer after transofmration.
///     
/// At the beginning of the connection, the function is called with a `NULL` buffer and zero `packet_number`.
/// At the end of the connection, the function is called with a `NULL` buffer and nonzero (i.e. the next) `packet_number`.
///
/// The function should return quickly. Websocat v1 is mostly single-threaded and pauses in this
/// function pause the whole Websocat (e.g. parallel connections for which this
/// transform function is supposed to be lightweight; or replies to WebSocket pings).
/// In Websocat v1 this function will be called from the same thread.
///
/// It is not possible to emit new data - if underlying connection does not return any data, this function is not called.
/// You cannot signal Websocat to wait and retry transforming (i.e. EAGAIN).
/// It may be problematic to fully absorb data as well - you can turn packets into zero-length, but those zero-length
/// packets would still be sent (or abort the connection, depending on options).
/// The function is supposed to be infallible - the only available error handling may be logging to stderr,
/// aborting the process or signaling errors using specially transformed data.
///
/// Transformation happes only on reads - writes just go though `transform:` overlay.
/// But you may use two distinct transforms on both left and right sides of Websocat command line.
/// `native_plugin_transform_a:mirror:` may be a good idea if you want to automatically reply to requests.
///
/// Actual symbol name is overridable from command line, so you may
/// want to copy and rename this prototype if you want to expose
/// multiple transforms from a single library
DLLEXPORT size_t websocat_transform(unsigned char* buffer, size_t data_length, size_t buffer_capacity, size_t connection_number, size_t packet_number);

/// This function is exported by Websocat and allow plugins to log data
/// Rust wants the data in the buffer to be UTF-8-formatted.
/// `severity`-ies are from 1 for "error" to 5 for "trace". 
DLLIMPORT void websocat_log(int severity, const char* buffer, size_t buffer_length);

#endif // WEBSOCAT_TRANSFORM_H
