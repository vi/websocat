#include <assert.h>
#include <stdio.h>

#include "websocat_plugin.h"

// Simulate `yes` tool, outputting "y\n" repeatedly.


WEBSOCAT_EXPORT int websocat_api_version() { return WEBSOCAT_API_VERSION; }

WEBSOCAT_EXPORT void* websocat_create_regular_sync(const char* restcmdline)
{
    fprintf(stderr, "websocat_create_regular_sync restcmdline=%s\n", restcmdline);
    return NULL;
}
WEBSOCAT_EXPORT void websocat_destroy_regular_sync(void* endpoint)
{
    assert(endpoint == NULL);
}

WEBSOCAT_EXPORT size_t websocat_sync_read (void* endpoint, void* buf, size_t buflen)
{
    assert(endpoint == NULL);
    assert(buflen >= 2);
    char* bufbuf = (char*)buf;
    bufbuf[0] = 'y';
    bufbuf[1] = '\n';
    return 2;
}

WEBSOCAT_EXPORT size_t websocat_sync_write(void* endpoint, const void* buf, size_t buflen)
{
    assert(endpoint == NULL);
    // Ignore everything
    return buflen;
}

