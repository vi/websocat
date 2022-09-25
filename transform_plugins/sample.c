#include <stdio.h>
#include <string.h>
#include "websocat_transform.h"

size_t websocat_transform(unsigned char* buffer, size_t data_length, size_t buffer_capacity, size_t connection_number, size_t packet_number)
{
   if (!buffer) {
      char buf[64];
      if (!packet_number) {
         sprintf(buf, "Connection %d started", (int)connection_number);
      } else {
         sprintf(buf, "Connection %d finished", (int)connection_number);
      }
      websocat_log(2, buf, strlen(buf));
      return 0;
   }
   size_t i;
   for (i=0; i<data_length; i+=1) {
      if (buffer[i] == '\n' || buffer[i] == '\r') continue;
      buffer[i] += 1;
   }
   return data_length;
}
