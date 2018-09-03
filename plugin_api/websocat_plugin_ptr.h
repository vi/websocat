#pragma once
#include <stdint.h>

// This file is just for bindgen input;
// maybe also for dlopening the plugins from C, if somebody wants so...

const uint32_t WEBSOCAT_API_VERSION = 1;

typedef uint32_t websocat_api_version();
typedef void* websocat_create_connection(const char* restcmdline);
typedef void websocat_destroy_connection(void* connection);
typedef int32_t websocat_read (void* connection, void* buf, uint32_t buflen);
typedef int32_t websocat_write(void* connection, const void* buf, uint32_t len);
typedef void* websocat_create_listener(const char* restcmdline);
typedef void websocat_destroy_listener(void* listener);
typedef void* websocat_get_connection_from_listener(void* listener);
typedef const char* websocat_global_aux(const char* request, const char* param);
typedef const char* websocat_connection_aux(void* connection, const char* request, const char* param);
typedef const char* websocat_listener_aux(void* listener, const char* request, const char* param);

#define WEBSOCAT_AUX_ORIENT "orientedness?"
    #define WEBSOCAT_AUX_ORIENT_MSG "MessageOriented"
    #define WEBSOCAT_AUX_ORIENT_STR "StreamOriented"


#define WEBSOCAT_AUX_DUPLEX "duplex?"
    #define WEBSOCAT_AUX_DUPLEX_HALF "half"
    #define WEBSOCAT_AUX_DUPLEX_FULL "full"


#define WEBSOCAT_AUX_CLIENT_ADDR "client_addr"
#define WEBSOCAT_AUX_URI "uri"
#define WEBSOCAT_AUX_CUSTOMPARAM_SET "customparam?"
#define WEBSOCAT_AUX_CUSTOMPARAM_GET "customparam"
#define WEBSOCAT_AUX_SHUTDOWN_READ  "shutdown_read"
#define WEBSOCAT_AUX_SHUTDOWN_WRITE "shutdown_write"

