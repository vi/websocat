#include <assert.h>
#include <stdio.h>

#include "websocat_plugin.h"

// Simulate `yes` tool, outputting "y\n" repeatedly.


WEBSOCAT_EXPORT uint32_t websocat_api_version() { return WEBSOCAT_API_VERSION; }

WEBSOCAT_EXPORT void* websocat_create_connection(const char* restcmdline)
{
    fprintf(stderr, "websocat_create_connection restcmdline=%s\n", restcmdline);
    return NULL;
}
WEBSOCAT_EXPORT void websocat_destroy_connection(void* endpoint)
{
    fprintf(stderr, "websocat_create_connection\n");
    assert(endpoint == NULL);
}

WEBSOCAT_EXPORT int32_t websocat_read (void* endpoint, void* buf, uint32_t buflen)
{
    assert(endpoint == NULL);
    assert(buflen >= 2);
    char* bufbuf = (char*)buf;
    bufbuf[0] = 'y';
    bufbuf[1] = '\n';
    return 2;
}

WEBSOCAT_EXPORT int32_t websocat_write(void* endpoint, const void* buf, uint32_t buflen)
{
    assert(endpoint == NULL);
    // Ignore everything
    return buflen;
}

