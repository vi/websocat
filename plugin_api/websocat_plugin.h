
/* 
 * C interface intended for writing custom specifiers (overlays or address types) for Websocat.
 * 
 * Currently it supports only endpoints (not overlays), only sync mode,
 * and only "connecting" (not listening and spawning new connections) although.
 * 
 * All functions related to one endpoint are called from one thread,
 * not the same as websocat's main thread. Multiple parallel connections = multiple threads.
 */

#ifndef WEBSOCAT_PLUGIN_H
#define WEBSOCAT_PLUGIN_H

#ifndef WEBSOCAT_EXPORT
#define WEBSOCAT_EXPORT
#endif

#include <stdint.h>

/// Bumped only on incompatible changes
#define WEBSOCAT_API_VERSION 1

/// Must just return WEBSOCAT_API_VERSION
WEBSOCAT_EXPORT uint32_t websocat_api_version();


/// Called by websocat when your regular (non-overlay) endpoint is being created
/// Returning NULL causes NULL to be supplied to read/write callbacks
/// restcmdline becomes invalid after you return from this function,
/// copy the data from it, not the pointer itself.
/// Listener plugins may omit this symbol.
WEBSOCAT_EXPORT void* websocat_create_connection(const char* restcmdline);
/// Called by websocat when your endpoing is not longer needed.
/// Listener plugins may omit this symbol.
WEBSOCAT_EXPORT void websocat_destroy_connection(void* connection);




/// Websocat requests data to be read from your endpoint.
/// Should block if no data is available.
/// Return 0 may mean EOF, otherwise return number of bytes you placed in the buffer.
/// There is no error reporting mechanism.
/// Return value is number of bytes actually processed from the buffer.
/// Retuning negative values means error. Error semantics should be like Linux's errnos,
/// for example, -1 (-EPERM) is "not permitted", -5 (-EIO) is "input/output error".
/// It is currently not specified what would happen if (-EWOULDBLOCK) or (-EINTR) is 
/// returned.
/// Write-only plugin-backed connections should return 0 on reads.
WEBSOCAT_EXPORT int32_t websocat_read (void* connection, void* buf, uint32_t buflen);

/// Websocat requests data to be written to your endpoint
/// Should block if congested
/// Don't return 0, return size of processed data.
/// Return value is number of bytes used from the buffer.
/// If it is less than `len`, then websocat_write will likely be repeated soon.
/// It is not recommended to return little values, as data is moved around in memory 
/// each write.
/// Negative return values mean errors, like with websocat_read.
/// Read-only plugin-backed connections should return len 
/// on write attempts, simulating /dev/null.
WEBSOCAT_EXPORT int32_t websocat_write(void* connection, const void* buf, uint32_t len);





/// Called by websocat when your listener endpoint is being created
/// Returning NULL means NULL will be supplied to `websocat_get_connection_from_listener`.
/// restcmdline becomes invalid after you return from this function.
/// Connection plugins may omit this symbol
WEBSOCAT_EXPORT void* websocat_create_listener(const char* restcmdline);
/// Called by websocat when your listener is not longer needed.
/// Connection plugins may omit this symbol
WEBSOCAT_EXPORT void websocat_destroy_listener(void* listener);


/// Expected to block and wait.
/// NULL means no more connections and we should
/// exit after finishing serving existing connections, if any
/// All spawned connections will have each own thread (or two).
WEBSOCAT_EXPORT void* websocat_get_connection_from_listener(void* listener);



/// This are "stringly typed" functions - depending on `request`,
/// there may be various param and return value meanings.
/// String returned by websocat_aux must be valid until
/// the next websocat_aux
/// call for the same object.
/// request=NULL means the last call (so can free up the buffer for aux responses)
/// Library may return NULL anytime it wants. It means no addition data / default settings.
/// Most API extensions are intended to happen as additional request types here, without 
/// touching the actual symbols and signatures.
/// All those functions may be called (with some unknown request) any time in a lifecycle
/// Those symbols may be omitted although, which is equivalent to always returning NULL.
WEBSOCAT_EXPORT const char* websocat_global_aux(const char* request, const char* param);
WEBSOCAT_EXPORT const char* websocat_connection_aux(void* connection, const char* request, const char* param);
WEBSOCAT_EXPORT const char* websocat_listener_aux(void* listener, const char* request, const char* param);


// The table of aux calls.
// param is assumed to be NULL if not mentioned
// return value is assumed to be ignored if not mentioned

// For websocat_global_aux:

#define WEBSOCAT_AUX_ORIENT "orientedness?"
    // stream-oriented means it will auto-insert line2message 
    // and message2line converters when talking to a text websocket
    // expected returns, default is message
    #define WEBSOCAT_AUX_ORIENT_MSG "MessageOriented"
    #define WEBSOCAT_AUX_ORIENT_STR "StreamOriented"


#define WEBSOCAT_AUX_DUPLEX "duplex?"
    // half duplex = one additional thread per connection (can't write while reading)
    // full duplex = two additional threads per connection (read and write)
    // expected returns, default is half
    #define WEBSOCAT_AUX_DUPLEX_HALF "half"
    #define WEBSOCAT_AUX_DUPLEX_FULL "full"


// For websocat_connection_aux:

#define WEBSOCAT_AUX_CLIENT_ADDR "client_addr"
    // param is client's ip:port, if known.
    // exact format is textual, but not defined precisely
#define WEBSOCAT_AUX_URI "uri"
    // param is URI if we accepted a websocket connection
#define WEBSOCAT_AUX_CUSTOMPARAM_SET "customparam?"
    // Called when plugin is specified on the left
    // (first positional argument).
    // param is ignored, return value is remembered.
    // May be available for CUSTOMPARAM_GET in other plugin
    // or as environment variable right part is exec: specifier.
#define WEBSOCAT_AUX_CUSTOMPARAM_GET "customparam"
    // Called when plugin is specified on the right
    // (second positional argument).
    // return value is ignored, param contains the value
    

// in full duplex mode, notify that some direction is finished, called from the respective thread.
#define WEBSOCAT_AUX_SHUTDOWN_READ  "shutdown_read"
#define WEBSOCAT_AUX_SHUTDOWN_WRITE "shutdown_write"



#endif // WEBSOCAT_PLUGIN_H
