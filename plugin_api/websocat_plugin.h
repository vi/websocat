
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

#include <stddef.h>

/// Bumped only on incompatible changes
/// Support of version 0 is going to be dropped if/when version 1 happens.
/// But version 1 may still be supported if/when version 2 comes.
#define WEBSOCAT_API_VERSION 0

/// Must just return WEBSOCAT_API_VERSION
WEBSOCAT_EXPORT int websocat_api_version();

/// Called by websocat when your regular (non-overlay) endpoint is being created
/// Returning NULL causes NULL to be supplied to read/write callbacks
/// restcmdline becomes invalid after you return from this function,
/// copy the data from it, not the pointer itself.
WEBSOCAT_EXPORT void* websocat_create_regular_sync(const char* restcmdline);
/// Called by websocat when your endpoing is not longer needed.
WEBSOCAT_EXPORT void websocat_destroy_regular_sync(void* endpoint);


/// Websocat requests data to be read from your endpoint.
/// Should block if no data is available.
/// Return 0 may mean EOF, otherwise return number of bytes you placed in the buffer.
/// There is no error reporting mechanism.
/// Return value is number of bytes actually processed from the buffer.
WEBSOCAT_EXPORT size_t websocat_sync_read (void* endpoint, void* buf, size_t buflen);

/// Websocat requests data to be written to your endpoint
/// Should block if congested
/// Don't return 0, return size of processed data.
/// There is no proper error reporting mechanism
/// Return value is number of bytes used from the buffer.
/// If it is less than `len`, then websocat_sync_write will likely be repeated soon.
/// It is not recommended to return little values, as data is moved around in memory 
/// each write.
WEBSOCAT_EXPORT size_t websocat_sync_write(void* endpoint, const void* buf, size_t len);



#endif // WEBSOCAT_PLUGIN_H
